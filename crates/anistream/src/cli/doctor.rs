//! `anistream --doctor`: the environment, reported without starting the interface. And
//! `anistream --refresh-data`, which brings the mapping data it reports on up to date.

use anistream_core::config::{Config, Paths};
use anistream_meta::dataset;
use anistream_net::HttpClient;
use anistream_providers::vpn::VpnGuard;
use anistream_store::Store;
use anistream_tui::theme;
use anyhow::Result;

use anistream_engine::{
    background::datasets::plain_failure, now_epoch, playback::player::old_mpv_note,
};

use crate::describe_graphics;

/// Report the environment, without starting the UI.
pub async fn doctor(config: &Config, paths: &Paths, store: &Store, images: bool) -> Result<()> {
    let engine = anistream_tui::image::ImageEngine::detect(images);
    let background = theme::detect::detect_background();
    let palette = theme::resolve_with(config.theme.mode, background);

    // First, since everything below it reads the configuration.
    println!("config");
    match Config::read(paths) {
        Ok(_) if !paths.config_file.exists() => {
            println!("  file        none yet — every setting at its default")
        }
        Ok(_) => println!("  file        {}", paths.config_file.display()),
        Err(problem) => {
            println!("  file        {}", paths.config_file.display());
            println!("              ✕ {problem}");
            println!("              not loaded: what follows is the defaults");
        }
    }
    println!();

    println!("terminal");
    println!("  graphics    {}", describe_graphics(engine.graphics()));
    let (cell_width, cell_height) = engine.font_size();
    println!("  cell size   {cell_width}×{cell_height} px");
    match background {
        Some(bg) => println!(
            "  background  #{:02X}{:02X}{:02X}  luminance {:.3}",
            bg.r,
            bg.g,
            bg.b,
            bg.luminance()
        ),
        None => println!("  background  not reported (OSC 11 unanswered)"),
    }
    println!(
        "  palette     {}",
        match palette.variant {
            theme::Variant::Dark => "adaptive, on a dark background",
            theme::Variant::Light => "adaptive, on a light background",
            theme::Variant::Immersive => "immersive",
        }
    );

    println!();
    println!("data");
    println!("  mappings    {} titles", store.mapping_count().unwrap_or(0));
    println!("  with offset {}", store.mappings_with_offset_count().unwrap_or(0));
    for spec in dataset::MAPPING_DATASETS {
        match store.dataset_state(spec.name) {
            Ok(Some(state)) => println!(
                "  {:<11} fetched {} · {} entries{}",
                spec.name,
                state.fetched_at.map_or("never".into(), describe_moment),
                state.item_count.unwrap_or(0),
                state
                    .last_error
                    .map_or(String::new(), |e| format!(" · last error: {}", plain_failure(&e)))
            ),
            _ => println!("  {:<11} never fetched", spec.name),
        }
    }

    // The player was missing from this report entirely, which is a strange omission for the one
    // external program the app cannot work without. Reported from real use: mpv accepted a URL
    // and then never rendered anything, and `--doctor` had nothing to say about it.
    println!();
    println!("player");
    let mpv = anistream_player::Mpv::new(std::env::temp_dir())
        .with_binary(config.playback.mpv_binary.clone())
        .with_extra_args(config.playback.mpv_args.clone());
    match mpv.version().await {
        Some(version) => {
            println!("  mpv         ● {version}");
            // mpv reads its own config, and a bad `vo` or `hwdec` there will hang playback in a
            // way anistream cannot distinguish from a dead source. Point at it by name.
            let conf = dirs_config_mpv();
            match conf.as_ref().filter(|p| p.exists()) {
                Some(path) => println!(
                    "  mpv.conf    {} — try `mpv --no-config <file>` if playback hangs",
                    path.display()
                ),
                None => println!("  mpv.conf    none"),
            }
            // Older mpv still plays, but drops parts of the player: say which, here as well as
            // at launch, so a degraded player has an explanation.
            if let Some(note) =
                anistream_player::protocol::MpvVersion::parse(&version).and_then(old_mpv_note)
            {
                println!("              {note}");
            }
        }
        None => {
            println!("  mpv         ✕ not runnable as {:?}", config.playback.mpv_binary);
            println!("              install it (brew install mpv), or set playback.mpv_binary");
        }
    }

    use anistream_core::config::PlayerUi;
    match config.playback.player_ui {
        PlayerUi::Anistream => {
            println!(
                "  interface   anistream (uosc, thumbnails, companion) — your mpv scripts are not loaded"
            )
        }
        PlayerUi::Mpv => println!("  interface   your mpv setup, plus the companion script"),
    }
    match anistream_player::Bundle::materialize(&paths.cache_dir) {
        Ok(bundle) => println!("  bundle      {}", bundle.root().display()),
        Err(e) => {
            println!("  bundle      ✕ could not be written: {e} — mpv's own look is used")
        }
    }
    // With the user's scripts loaded, another script can take keys or tracks ours expect.
    // Listing them is enough for a human to spot the one that fights.
    if config.playback.player_ui == PlayerUi::Mpv
        && let Some(dir) =
            dirs_config_mpv().and_then(|conf| conf.parent().map(|d| d.join("scripts")))
        && let Ok(entries) = std::fs::read_dir(&dir)
    {
        let mut names: Vec<String> =
            entries.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        if !names.is_empty() {
            println!("  scripts     {} — {}", dir.display(), names.join(", "));
        }
    }

    println!();
    println!("sources");
    if !config.providers.torrent.enabled {
        println!("  torrent     disabled (set providers.torrent.enabled and a vpn mode)");
        return Ok(());
    }

    println!(
        "  torrent     enabled, vpn mode {}",
        match config.providers.torrent.vpn.mode {
            anistream_core::config::VpnMode::Socks5 => "socks5",
            anistream_core::config::VpnMode::External => "external",
            anistream_core::config::VpnMode::None => "none",
        }
    );

    if config.providers.torrent.trackers.is_empty() {
        println!(
            "  trackers    none configured — DHT is off in proxy mode, so no peers will be found"
        );
    } else {
        println!("  trackers    {} configured", config.providers.torrent.trackers.len());
    }

    // Actually run the guard. Reporting the configured mode without checking it would be
    // useless in the one situation where you reach for this command.
    match VpnGuard::new(config.providers.torrent.vpn.clone()) {
        Ok(guard) => {
            print!("  verifying egress through the proxy… ");
            use std::io::Write;
            let _ = std::io::stdout().flush();

            let state = guard.verify().await;
            println!("{}", state.badge());
            match state.reason() {
                Some(reason) => println!("  ✕ torrent source will NOT start: {reason}"),
                None => println!("  ● torrent source will start"),
            }
            if guard.must_disable_dht() {
                println!("  note        DHT disabled in proxy mode (UDP-associate unverified)");
            }

            // The distinction that actually matters. Everything above is application-level
            // and can be defeated by a bug here; only a firewall rule makes leaking
            // impossible the way an interface bind does.
            let enforcement = anistream_providers::vpn::detect_kernel_enforcement().await;
            println!();
            println!("  os-level kill switch");
            match enforcement {
                anistream_providers::vpn::KernelEnforcement::Enforced => {
                    println!("    ● ENFORCED — {}", enforcement.advice());
                }
                _ => {
                    println!("    ▲ {}", enforcement.advice());
                    println!();
                    println!("    Without it, anistream's guard is defence in depth, not a");
                    println!(
                        "    guarantee: it stops anistream leaking, but cannot stop a bug"
                    );
                    println!("    in anistream or any other process on this machine.");
                }
            }
        }
        Err(reason) => println!("  ✕ vpn guard misconfigured: {reason}"),
    }
    Ok(())
}

pub async fn refresh_data(store: &Store, http: &HttpClient) -> Result<()> {
    let now = now_epoch();
    for (name, outcome) in dataset::refresh_all(store, http, now, true).await {
        println!("{name:<12} {}", describe_refresh(&outcome));
    }
    println!("{} titles mapped", store.mapping_count().unwrap_or(0));
    Ok(())
}

/// A moment, as a local date and time and how long ago that was.
fn describe_moment(epoch: i64) -> String {
    describe_moment_in(epoch, now_epoch(), &jiff::tz::TimeZone::system())
}

/// [`describe_moment`] against a given now and zone. Days are counted between calendar dates
/// where the reader is: counting whole 86,400 s blocks called last night's refresh "today".
fn describe_moment_in(epoch: i64, now: i64, zone: &jiff::tz::TimeZone) -> String {
    let (Ok(at), Ok(now)) =
        (jiff::Timestamp::from_second(epoch), jiff::Timestamp::from_second(now))
    else {
        return "at an unreadable time".into();
    };
    let local = at.to_zoned(zone.clone());
    let today = now.to_zoned(zone.clone()).date();
    let days = local.date().until(today).map_or(0, |span| span.get_days()).max(0);
    let ago = match days {
        0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        n => format!("{n} days ago"),
    };
    format!("{} ({ago})", local.strftime("%Y-%m-%d %H:%M"))
}

/// One dataset refresh's outcome, as a person reads it.
fn describe_refresh(outcome: &dataset::RefreshOutcome) -> String {
    match outcome {
        dataset::RefreshOutcome::NotDue => "current, not due for a check".into(),
        dataset::RefreshOutcome::Unchanged => "unchanged since the last fetch".into(),
        dataset::RefreshOutcome::Updated { entries } => format!("updated, {entries} entries"),
        dataset::RefreshOutcome::Failed { reason } => {
            format!("failed: {}", plain_failure(reason))
        }
    }
}

/// Where mpv keeps its own configuration, if it is in the usual place.
///
/// Not authoritative — mpv honours `MPV_HOME` and a portable-config directory too — so this is a
/// pointer for the human reading `--doctor`, not something the app acts on.
fn dirs_config_mpv() -> Option<std::path::PathBuf> {
    if let Ok(home) = std::env::var("MPV_HOME") {
        return Some(std::path::PathBuf::from(home).join("mpv.conf"));
    }
    std::env::var_os("HOME")
        .map(|home| std::path::PathBuf::from(home).join(".config/mpv/mpv.conf"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_is_dated_by_the_calendar_where_the_reader_is() {
        let zone = jiff::tz::TimeZone::fixed(jiff::tz::offset(2));
        let at = |s: &str| {
            s.parse::<jiff::civil::DateTime>().unwrap().to_zoned(zone.clone()).unwrap()
        };
        let epoch = |s: &str| at(s).timestamp().as_second();
        let now = epoch("2026-10-18T09:00");
        // Twelve hours ago, but before midnight.
        assert_eq!(
            describe_moment_in(epoch("2026-10-17T21:00"), now, &zone),
            "2026-10-17 21:00 (yesterday)"
        );
        assert_eq!(
            describe_moment_in(epoch("2026-10-18T00:30"), now, &zone),
            "2026-10-18 00:30 (today)"
        );
        assert_eq!(
            describe_moment_in(epoch("2026-10-15T23:59"), now, &zone),
            "2026-10-15 23:59 (3 days ago)"
        );
    }

    #[test]
    fn a_dataset_refresh_is_described_in_words() {
        use dataset::RefreshOutcome;
        let unreachable = RefreshOutcome::Failed {
            reason: "error sending request for url (https://datasets.example/mapping.json)"
                .into(),
        };
        assert_eq!(describe_refresh(&unreachable), "failed: the server could not be reached");
        let refused = RefreshOutcome::Failed { reason: "HTTP 503".into() };
        assert_eq!(describe_refresh(&refused), "failed: HTTP 503");
        assert_eq!(
            describe_refresh(&RefreshOutcome::Updated { entries: 12 }),
            "updated, 12 entries"
        );
        for outcome in [unreachable, refused, RefreshOutcome::NotDue, RefreshOutcome::Unchanged]
        {
            let said = describe_refresh(&outcome);
            assert!(!said.contains('{') && !said.contains("Failed"), "Debug text: {said}");
        }
    }
}
