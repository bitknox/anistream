//! What anistream's engine and its front ends say to each other.
//!
//! Every request a front end can make and everything it is told back, as plain values: the
//! commands it sends, the events it receives, the rows those carry, and the words the app uses for
//! them. The terminal is one front end; a native app on the same engine is meant to be another,
//! and each renders the same values its own way. So nothing here knows how it is shown.
//!
//! What belongs here:
//! - data that crosses between the engine and a front end;
//! - the pure rules over that data, such as where a title's progress is counted against;
//! - the words for it that every front end would otherwise write again: a count and its noun, a
//!   size at human scale, a countdown.
//!
//! What never does:
//! - I/O or a runtime: no tokio, no files, no network, and no clock read here. A moment arrives
//!   as a value, a span or a Unix time, and whoever draws it counts it down.
//! - Anything about a terminal: no ratatui or crossterm, no glyphs, no lifetimes counted in
//!   frames, no keys. A front end's view state stays in that front end.
//! - The engine's own state, its services, or how it decides.
//!
//! The `ffi` feature derives UniFFI records and enums on the types, and the core's on those of its
//! types they carry, for a native front end's bindings. Every type crosses, and CI holds them to
//! it: the feature's build fails on one that cannot, such as a tuple, a borrowed string or a
//! `Result` in a field. What the build cannot see is a clash in the Kotlin bindings, where each
//! variant is a class nested in its enum's and hides any type of its name spelled bare there, such
//! as Kotlin's own `List`. So no variant is named after a type it carries, nor after a Kotlin
//! type. Where the type is the model's, the type is the one renamed, so the variant keeps the name
//! the front ends match on.
//!
//! **Adding a field to a record.** Kotlin's generated constructor and Swift's memberwise init
//! take every field, and the apps build records of their own (placeholders, previews, tests),
//! so a new field breaks both apps' builds unless the bindings give it a default:
//! - An `Option` or `Vec` field carries `#[cfg_attr(feature = "ffi", uniffi(default))]`, which
//!   is `None` or empty. A test below fails on one without it.
//! - So does a `bool` that is off until something sets it, and a meter that starts at nought.
//! - What a record is about stays required: an id, a title, a row's value.
//! - A new field goes last. A Kotlin call that passes arguments by position, as some tests do,
//!   keeps compiling only then.
//!
//! A field with no natural default is a change to the apps, made in the same PR. Build them:
//! `android/scripts/engine.sh testing` then `./gradlew :ui:compileFixturesDebugKotlin
//! :store:testFixturesDebugUnitTest` in `android/`, and `apple/build.sh`.

pub mod bug_report;
pub mod catalog;
pub mod command;
pub mod discussion;
pub mod downloads;
pub mod episodes;
pub mod event;
pub mod hosted;
pub mod jobs;
#[rustfmt::skip]
mod msg;
pub mod msg_text;
pub mod notice;
pub mod notifications;
pub mod party;
pub mod playback;
pub mod present;
pub mod progress;
pub mod protection;
pub mod remote;
pub mod search;
pub mod settings;
pub mod show;
pub mod sources;
pub mod stores;
pub mod sync;
pub mod trailer;
pub mod updates;
pub mod widgets;
pub mod wording;

pub use msg::{Cause, Msg};
pub use msg_text::{Size, SizeUnit, Span};

// What the types' UniFFI derives register with, under the `ffi` feature only. It compiles under the
// workspace's `unsafe_code = "forbid"`: the lint is not reported inside another crate's macros,
// and every unsafe block and exported symbol here comes out of UniFFI's.
#[cfg(feature = "ffi")]
uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    /// The rule in the crate's docs, read from the sources: every module's, so a new one is held
    /// to it too. The attribute goes right before its field, under the field's docs.
    #[test]
    fn every_option_and_vec_field_of_a_record_has_a_default_in_the_bindings() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<_> = fs::read_dir(&src)
            .expect("the crate's sources")
            .map(|entry| entry.expect("a source file").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
            .collect();
        files.sort();
        assert!(files.len() > 10, "found the modules: {files:?}");

        let mut missing = Vec::new();
        for path in &files {
            let source = fs::read_to_string(path).expect("a readable source");
            let file = path.file_name().unwrap().to_string_lossy();
            let lines: Vec<&str> = source.lines().collect();
            let (mut derived, mut record) = (false, None);
            for (i, line) in lines.iter().enumerate() {
                if line.contains("derive(uniffi::Record)") {
                    derived = true;
                } else if let Some(rest) = line.strip_prefix("pub struct ") {
                    record =
                        derived.then(|| rest.split([' ', '{', '<']).next().unwrap_or(rest));
                    derived = false;
                } else if *line == "}" {
                    record = None;
                } else if let (Some(name), Some(field)) =
                    (record, line.strip_prefix("    pub "))
                {
                    let Some((field, ty)) = field.split_once(": ") else { continue };
                    let defaulted = lines[i - 1].contains("uniffi(default");
                    if (ty.starts_with("Option<") || ty.starts_with("Vec<")) && !defaulted {
                        missing.push(format!("{file}: {name}.{field}"));
                    }
                }
            }
        }
        assert!(
            missing.is_empty(),
            "these fields need #[cfg_attr(feature = \"ffi\", uniffi(default))], or the apps that \
             build their records stop compiling: {missing:#?}"
        );
    }
}
