//! `anistream --stream-url`, `--play` and `--continue`: one episode resolved to a URL to test
//! by hand, or played in mpv with no interface around it.

use anistream_core::config::{Config, Paths};
use anistream_engine::{
    playback::{
        self,
        player::{player, shader_dir},
        slot::PlayerSlot,
    },
    sources, tracking,
};
use anistream_model::event::Event;
use anistream_net::HttpClient;
use anistream_store::Store;
use anistream_tui::theme::player_theme;
use anyhow::{Context, Result};
use tokio::sync::mpsc;

use crate::{
    now_epoch, play_settings, resolve_for_playback, services::build_meta, signed_in_tracker_ids,
};

/// Resolve one episode and print the URL, keeping the stream server alive.
///
/// Exists because a silent playback failure gave the user nothing to test with. Splitting
/// "anistream cannot resolve this" from "my player cannot play it" needs a URL in hand, and a
/// torrent-backed URL only serves while the session that owns it is running.
pub async fn stream_url(
    config: &Config,
    http: &HttpClient,
    store: &Store,
    paths: &Paths,
    id: u32,
    episode: &str,
) -> Result<()> {
    use anistream_core::ids::AnilistId;

    let id = AnilistId::new(id);
    let meta = build_meta(config, http, store);

    let started = std::time::Instant::now();
    let (registry, guard, note, _) = sources::build_registry(config, http, paths, store).await;
    println!("sources     {:?} in {:?}", registry.ids(), started.elapsed());
    if let Some(note) = &note {
        println!("note        {note}");
    }
    if registry.is_empty() {
        anyhow::bail!("no sources registered — see --doctor");
    }

    let media = meta.media(id).await.context("looking up the title")?;
    println!("title       {}", media.title.display());

    let now = now_epoch();
    let translation = config.playback.translation;
    let answer = anistream_providers::first_answer(
        store,
        &registry,
        id,
        &media.match_target(),
        translation,
        now,
        |source, key| async move { source.resolve(&key, episode, translation, now).await },
    )
    .await;
    let streams = match answer {
        anistream_providers::Answer::Answered { provider_id, key, value } => {
            println!("matched     {key:?} on {provider_id}");
            value
        }
        anistream_providers::Answer::Ambiguous { .. } => {
            anyhow::bail!("could not match this title: {}", answer.explain())
        }
        anistream_providers::Answer::Unanswered { .. } => {
            anyhow::bail!("every source failed: {}", answer.explain())
        }
    };

    println!();
    for (i, stream) in streams.iter().enumerate() {
        println!("[{i}] {:?}  {:?}", stream.kind, stream.quality);
        println!("    {}", stream.url);
        for (name, value) in &stream.headers {
            println!("    header  {name}: {value}");
        }
    }

    println!();
    println!(
        "Try it directly — if these fail, the player or the source is at fault, not anistream:"
    );
    println!("  mpv --no-config '{}'", streams[0].url);
    println!("  curl -sI '{}'", streams[0].url);
    println!();
    if guard.is_some() {
        println!("The torrent session is serving this URL. Press Ctrl-C when you are done.");
        tokio::signal::ctrl_c().await.ok();
    }
    Ok(())
}

/// Play one episode with no TUI around it.
///
/// The full playback session — resume, aniskip, chapters, history, tracker queueing —
/// through the same `playback::play` the app calls, with its updates narrated to stdout.
/// What ends it is mpv exiting, exactly like closing the player ends a TUI session.
pub async fn play_cli(
    config: &Config,
    http: &HttpClient,
    store: &Store,
    paths: &Paths,
    id: anistream_core::ids::AnilistId,
    episode: String,
) -> Result<()> {
    let meta = build_meta(config, http, store);

    let (registry, guard, note, _) = sources::build_registry(config, http, paths, store).await;
    if let Some(note) = &note {
        println!("note        {note}");
    }
    if registry.is_empty() {
        anyhow::bail!("no sources registered — see --doctor");
    }
    // Kept alive for the whole session: dropping the VPN guard would drop its checks.
    let _guard = guard;

    let mpv = player(config, paths, &player_theme());
    if !mpv.is_available().await {
        anyhow::bail!(
            "{} not found — install mpv to play anything",
            config.playback.mpv_binary
        );
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    // Events only: the command line narrates them, and has no engine side to tell anything.
    let tx = anistream_engine::outbox::Outbox::from(tx);
    let (streams, context) = resolve_for_playback(
        &meta,
        store,
        &registry,
        id,
        &episode,
        None,
        &config.playback,
        None,
        &tx,
    )
    .await
    .map_err(|reason| anyhow::anyhow!(reason))?;

    println!("playing     {} ep {}", context.title, context.episode);
    if let Some(at) = context.resume_at {
        println!("resuming    {}m{:02}s in", (at / 60.0) as u32, (at % 60.0) as u32);
    }

    // The updates the TUI would draw, printed instead. Drained concurrently so a full
    // channel can never stall the player.
    let printer = tokio::spawn(async move {
        let mut watched = false;
        while let Some(update) = rx.recv().await {
            match update {
                Event::Status(s) if !s.trim().is_empty() => println!("{s}"),
                Event::Toast(t) => println!("{}", t.text),
                Event::PlaybackEnded { watched: w, .. } => watched = w,
                _ => {}
            }
        }
        watched
    });

    // The command channel stays open but silent — headless has no keys to forward. The
    // sender must outlive the session: a closed channel reads as controls torn down.
    let (_ptx, prx) = mpsc::unbounded_channel();
    // The same queue set the TUI uses: signed-in trackers, not merely enabled ones.
    let sync = tracking::Sync::build(config, store, http);
    let tracker_ids = signed_in_tracker_ids(&sync);
    let settings = playback::PlaySettings {
        shader_dir: shader_dir(config, paths),
        ..play_settings(config, tracker_ids.clone(), config.syncplay.enabled)
    };
    // Headless plays one episode: nothing would ever claim a parked window.
    let settings = playback::PlaySettings { auto_next: false, ..settings };
    playback::play(
        streams,
        context,
        store.clone(),
        http.clone(),
        mpv,
        settings,
        PlayerSlot::default(),
        tx,
        prx,
    )
    .await;

    let watched = printer.await.unwrap_or(false);
    if watched && tracker_ids.is_empty() {
        println!("counted as watched");
    } else if watched {
        println!("counted as watched — queued for {}", tracker_ids.join(", "));
    } else {
        println!("stopped early — the position is saved, --continue picks it back up");
    }
    Ok(())
}
