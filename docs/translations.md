# Translations

Every string a person reads comes from one Fluent catalogue in `i18n/`. `tools/i18n` turns it into
each platform's own format. The generated files are committed, and CI fails when one is stale.

Never edit a generated file. Edit the catalogue and run:

```
cargo run -p i18n-tool -- generate
```

## Add a string

1. Pick the file in `i18n/en/` by screen (`ui-library.ftl`, `ui-settings.ftl`, and so on). Strings
   that only one platform shows go in `platform-<name>.ftl`. Engine notices go in `engine.ftl`.
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

## Pseudo-locale

Accented, 30% longer English for checking layouts and finding strings that were never moved.

- Apple: the scheme option "Double-Length Pseudolanguage", or `-AppleLanguages (en-XA)`.
- Android: debug builds enable `en-XA` and `ar-XB` in the system language list.
- Windows: the `qps-ploc` language.
- Linux and TUI: `ANISTREAM_LANG=en-XA` (debug builds, or the `pseudo` feature).

## Lints

`lint` holds the catalogue to the writing rules (glossary terms, no em dashes, no all caps, no
"provider", consistent placeholders and plurals, widths). `lint-code` counts hardcoded strings per
platform against `i18n/baselines/<platform>.txt`. It only reports until a platform is added to
`[code] enforce` in `i18n/lint.toml`, which that platform's migration does once its baseline is
empty. Platform hooks: the CI `i18n` job, `apple/scripts/test.sh`, Android's Gradle `check` and
`desktop/windows/scripts/test.sh`, each running `lint-code --platform <name>`.
