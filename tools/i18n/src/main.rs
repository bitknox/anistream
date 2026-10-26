use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use i18n_tool::catalogue::{Catalogue, Platform};
use i18n_tool::lint::{self, Severity};
use i18n_tool::ship::Mode;
use i18n_tool::{codelint, find_root, generate_all, pseudo, stale, write};

/// anistream's translations: one Fluent catalogue in i18n/, every platform's strings generated.
#[derive(Parser)]
#[command(name = "i18n", version)]
struct Cli {
    /// The repository root; found from the current directory when not given.
    #[arg(long, global = true)]
    root: Option<std::path::PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    Release,
    Dev,
}

#[derive(Subcommand)]
enum Command {
    /// Write every platform's strings from the catalogue.
    Generate {
        /// `release` ships reviewed strings (and drafts of languages that ship them); `dev` adds all drafts.
        #[arg(long, value_enum, default_value = "release")]
        mode: ModeArg,
    },
    /// Fail when a generated file differs from what the catalogue produces now.
    Check,
    /// Check the catalogue's words against the copy rules, in every language.
    Lint,
    /// Count string literals the apps still hardcode, against the baselines. Reports; fails only for platforms listed under [code] enforce in i18n/lint.toml.
    LintCode {
        #[arg(long)]
        platform: Option<String>,
        /// Rewrite the baselines to what is there now (they may only shrink).
        #[arg(long)]
        update_baseline: bool,
    },
    /// Print TEXT as the pseudo-locale writes it (accented, 30% longer, bracketed).
    Pseudo { text: String },
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    let root = match cli.root {
        Some(r) => r,
        None => find_root(&std::env::current_dir()?).context(
            "no i18n/languages.toml above here; run from the repository or pass --root",
        )?,
    };
    match cli.command {
        Command::Pseudo { text } => {
            println!("{}", pseudo::text(&text));
            Ok(ExitCode::SUCCESS)
        }
        Command::Generate { mode } => {
            let cat = Catalogue::load(&root)?;
            let files = generate_all(&cat, mode_of(mode))?;
            let changed = write(&root, &files)?;
            println!("{} files generated, {changed} changed", files.len());
            Ok(ExitCode::SUCCESS)
        }
        Command::Check => {
            let cat = Catalogue::load(&root)?;
            let problems = stale(&root, &generate_all(&cat, Mode::Release)?);
            let mut failed = false;
            for f in lint::status_findings(&cat)? {
                eprintln!("{f}");
                failed |= f.severity == Severity::Error;
            }
            if !problems.is_empty() {
                eprintln!("The generated strings are out of date:");
                for p in &problems {
                    eprintln!("  {p}");
                }
                eprintln!("Run `cargo run -p i18n-tool -- generate` and commit the result.");
                failed = true;
            }
            if failed {
                return Ok(ExitCode::FAILURE);
            }
            println!("The generated strings match the catalogue.");
            Ok(ExitCode::SUCCESS)
        }
        Command::Lint => {
            let cat = Catalogue::load(&root)?;
            let findings = lint::run(&cat)?;
            for f in &findings {
                eprintln!("{f}");
            }
            let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
            if errors > 0 {
                eprintln!("{errors} errors");
                return Ok(ExitCode::FAILURE);
            }
            println!("The catalogue follows the copy rules ({} warnings).", findings.len());
            Ok(ExitCode::SUCCESS)
        }
        Command::LintCode { platform, update_baseline } => {
            let cat = Catalogue::load(&root)?;
            let config = lint::load_config(&cat)?;
            let platforms: Vec<Platform> = match platform {
                Some(p) => {
                    vec![Platform::parse(&p).with_context(|| format!("no platform {p}"))?]
                }
                None => Platform::ALL.to_vec(),
            };
            let mut failed = false;
            for p in platforms {
                let counts = codelint::scan(&root, p)?;
                if update_baseline {
                    codelint::write_baseline(&root, p, &counts)?;
                    println!(
                        "{:8} baseline written: {} literals in {} files",
                        p.name(),
                        counts.values().sum::<usize>(),
                        counts.len()
                    );
                    continue;
                }
                let report = codelint::compare(&counts, &codelint::read_baseline(&root, p));
                let enforced = config.code.enforce.iter().any(|e| e == p.name());
                println!(
                    "{:8} {} hardcoded literals in {} files (baseline {}){}",
                    p.name(),
                    report.total,
                    counts.len(),
                    report.baseline,
                    if enforced { ", enforced" } else { ", report only" }
                );
                for (file, n, allowed) in &report.grown {
                    println!("  {file}: {n}, baseline allows {allowed}");
                    failed |= enforced;
                }
            }
            if failed {
                bail!(
                    "new hardcoded strings; add them to the catalogue (docs/translations.md)"
                );
            }
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn mode_of(m: ModeArg) -> Mode {
    match m {
        ModeArg::Release => Mode::Release,
        ModeArg::Dev => Mode::Dev,
    }
}
