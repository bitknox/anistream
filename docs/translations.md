# Translations

Every string a person reads comes from one Fluent catalogue in `i18n/`. `tools/i18n` turns it into
each platform's own format. The generated files are committed, and CI fails when one is stale.

Never edit a generated file. Edit the catalogue and run:

```
cargo run -p i18n-tool -- generate
```

## Add a string

1. Pick the file in `i18n/en/` by screen (`ui-library.ftl`, `ui-settings.ftl`, and so on). Strings
   that only one platform shows go in `platform-<name>.ftl` (or `platform-<name>-<screen>.ftl`). Engine notices go in `engine.ftl`.
2. Write the message. The id is kebab-case and names what the text is, not where it sits.

   ```
   # Continue row subtitle. The show's title is cut first when the row is short.
   # @arg count: int
   # @arg title: text
   # @max 40
   queue-episodes-left = { $count ->
       [one] One episode left in { $title }
      *[other] { $count } episodes left in { $title }
    }
   ```

   The comment is the translator's context. Declare every argument with `# @arg name: type` (text,
   int, decimal, percent, seconds, bytes, timestamp). A plural selects on an int argument, uses CLDR
   categories, and ends with the `*[other]` default. Use `@max N` for a width limit and
   `@only apple|android|windows|linux|tui` when only some platforms show it.
3. Use the glossary's words through terms (`{ -source }`, `{ -plugin-cap-pl }`), never typed out.
   `i18n/en/terms.ftl` lists them. Fluent terms have no attributes, so each form is its own term.
4. Run `generate`, then `lint` and `check`. Commit the catalogue and the generated files together.

The subset of Fluent in use is checked by the loader: no functions, no message references, one
plural select per message.

## Use a key

| Platform | Reference |
| --- | --- |
| Apple | `import AnistreamStrings`, then `L.settingsLanguage` or `L.queueEpisodesLeft(count: 3, title: t)` |
| Android | `stringResource(R.string.settings_language)` from `tv.anistream.i18n.R`; plurals through `pluralStringResource` or the helpers in `Strings.kt` |
| Windows | `T.SettingsLanguage`, `T.QueueEpisodesLeft(3, title)` (Anistream.App.Core); in XAML, `x:Uid` against the `.resw` keys |
| Linux, TUI, engine | `anistream_i18n::t::settings_language()`; call `anistream_i18n::init(saved)` once at start-up |

A missing key is a compile error on every platform except XAML `x:Uid`. A language that lacks a
message shows it in English, per string.

## Languages and review

`i18n/languages.toml` lists the languages, each with a stage and a draft policy. A translation
ships only when `i18n/status/<lang>.json` marks it `reviewed` against the current English text. A
language set to `drafts = "ship"` also ships drafts, and the apps label it "Draft translation".

## Draft a language

`i18n-tool -- draft <lang>` is how machine drafts get in. The language must be in `languages.toml`
with a folder `i18n/<lang>/`.

```
cargo run -q -p i18n-tool -- draft sv --list --group ui-library > batch.json
cargo run -q -p i18n-tool -- draft sv --accept proposals.json --group ui-library
```

`--list` prints the missing and stale messages as JSON, one batch per screen file: the English
as Fluent source, the translator comment, `@arg` list, `@max`, `@only`, the plural categories the
language needs, the terms the message uses, and the old draft when the English changed. `terms`
is a batch of its own (ids such as `-source`) and comes first for a new language.

`--accept` takes `{ "<id>": "<translation>" }`. A plural is a map of its forms
(`{ "one": "...", "other": "..." }`). It refuses, with a reason per id, a translation that does
not parse, changes the placeables, lacks a plural category of the language, types a glossary
word that has a term, has an em dash, is longer than `@max`, or names something on the lint's
banned list. The rest are written to `i18n/<lang>/<file>.ftl` and recorded in
`i18n/status/<lang>.json` as `draft` with the hash of the English. The exit code is 1 when anything
was refused. `legal-hosts-no-content` is never offered to an agent. Terms have no review state;
the language's terms file is approved as a whole before its messages are drafted.

`--drafts` prints every stored draft next to its English for a review pass (`--group` narrows it).
`--notes FILE` takes `{ "<id>": { "back": "...", "flags": ["..."] } }` and stores a back-translation
and review flags with the entry; an absent key leaves the stored value alone and `"flags": null`
clears them. Parallel runs on one language are safe: each write takes a lock and replaces the file
atomically.

Tags are BCP 47 as written in `languages.toml` (`zh-Hans`, `es-419`, `pt-BR`). Android writes
them as `values-b+zh+Hans`, Windows as `Strings/zh-Hans`, Apple and Fluent keep them as they are.
Some languages need a `many` form that only counts millions (es, pt, fr, it); `--list` names the
categories.

## Language in the terminal

`--lang <tag>` wins over the `language` key in `config.toml`, and both win over the system's
language. `ANISTREAM_LANG` still overrides all three, for trying a language or the pseudo-locale.

## Pseudo-locale

Accented, 30% longer English for checking layouts and finding strings that were never moved.

- Apple: the scheme option "Double-Length Pseudolanguage", or the launch arguments
  `-NSDoubleLocalizedStrings YES -NSAccentuateLocalizedStrings YES`. `-AppleLanguages (en-XA)` shows
  plain English, because the app has no en-XA localisation. This doubles the length, which is harsher
  than the 30% of the other platforms, and it also mangles `%1$@` style arguments in the output.
  `ANISTREAM_LAUNCH_ARGS="..." apple/scripts/shots.sh iphone` passes them to the Simulator shots.
  Rail titles and sort names come from the engine and stay English here.
- Android: debug builds enable `en-XA` and `ar-XB` in the system language list.
- Windows: the `qps-ploc` language.
- Linux and TUI: `ANISTREAM_LANG=en-XA` (debug builds, or the `pseudo` feature).

## Lints

`lint` holds the catalogue to the writing rules (glossary terms, no em dashes, no all caps, no
"provider", consistent placeholders and plurals, widths). `lint-code` counts hardcoded strings per
platform against `i18n/baselines/<platform>.txt`. For the terminal it counts every string literal that reads as words (three letters in a row) outside logs, assertions, match patterns and comparisons. On every platform a line ending in `// i18n-skip` marks a literal that is a token, not text (an id, a symbol name, a format code), and `I18N_LIST=1` prints each one found. It only reports until a platform is added to
`[code] enforce` in `i18n/lint.toml`, which that platform's migration does once its baseline is
empty. Platform hooks: the CI `i18n` job, `apple/scripts/test.sh`, Android's Gradle `check` and
`desktop/windows/scripts/test.sh`, each running `lint-code --platform <name>`.
