//! `anistream --stats`, `--export`, `--import` and `--random`: the local watch history, read,
//! written out, merged back in, and picked from.

use anistream_store::Store;
use anyhow::{Context, Result};

pub fn stats_cli(store: &Store, json: bool) -> Result<()> {
    let stats = store.stats()?;

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "titles": stats.titles,
                "episodes_completed": stats.episodes_completed,
                "episodes_started": stats.episodes_started,
                "watched_secs": stats.watched_secs,
                "watched_human": stats.watched_human(),
                "first_at": stats.first_at,
                "last_at": stats.last_at,
                "top_provider": stats.top_provider.as_ref().map(|(id, n)| {
                    serde_json::json!({ "id": id, "episodes": n })
                }),
            }))?
        );
        return Ok(());
    }

    if stats.titles == 0 {
        println!("Nothing watched yet.");
        return Ok(());
    }
    println!("  titles          {}", stats.titles);
    println!("  episodes        {} finished", stats.episodes_completed);
    println!("  started         {}", stats.episodes_started);
    println!("  watch time      {}", stats.watched_human());
    if let Some((provider, count)) = &stats.top_provider {
        println!("  mostly via      {provider} ({count} episodes)");
    }
    Ok(())
}

/// Write an export. `-` means stdout, so it composes with a pipe.
pub fn export_cli(store: &Store, path: &str) -> Result<()> {
    let export = store.export(anistream_store::now())?;
    let json = serde_json::to_string_pretty(&export)?;

    if path == "-" {
        println!("{json}");
    } else {
        std::fs::write(path, &json).with_context(|| format!("writing the export to {path}"))?;
        eprintln!("wrote {} titles to {path}", export.titles.len());
    }
    Ok(())
}

pub fn import_cli(store: &Store, path: &str) -> Result<()> {
    let raw = if path == "-" {
        std::io::read_to_string(std::io::stdin()).context("reading the export from stdin")?
    } else {
        std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?
    };
    let export: anistream_store::stats::Export =
        serde_json::from_str(&raw).context("parsing the export")?;

    let advanced = store.import(&export, anistream_store::now())?;
    // Progress is monotonic, so an unchanged count is a normal outcome rather than a failure —
    // re-importing the same file, or restoring an older backup, both land here.
    println!(
        "imported {} of {} titles ({} already up to date)",
        advanced,
        export.titles.len(),
        export.titles.len().saturating_sub(advanced as usize)
    );
    Ok(())
}

pub fn random_cli(store: &Store, json: bool) -> Result<()> {
    let Some(id) = store.random_watched()? else {
        if json {
            println!("null");
        } else {
            println!("Nothing in your history yet.");
        }
        return Ok(());
    };
    let title = store.cached_title(id).unwrap_or(None);
    let progress = store.progress(id).ok().flatten();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "anilist_id": id.get(),
                "title": title,
                "episodes_completed": progress.as_ref().map(|p| p.episodes_done),
                // A string, as every episode is in this output and the export: episodes are
                // named, not counted — `OVA` and `12.5` are episodes too. Counts stay numbers.
                "next_episode": progress.as_ref().map(|p| (p.episodes_done + 1).to_string()),
            }))?
        );
    } else {
        let name = title.unwrap_or_else(|| format!("anilist {}", id.get()));
        match progress {
            Some(p) => println!("{name} — next up: episode {}", p.episodes_done + 1),
            None => println!("{name}"),
        }
    }
    Ok(())
}
