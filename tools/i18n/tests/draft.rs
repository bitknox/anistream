//! `draft <lang>`: what the list tells an agent, and what `accept` writes and refuses.

use std::fs;
use std::path::Path;

use i18n_tool::catalogue::Catalogue;
use i18n_tool::draft::{accept, list};
use i18n_tool::ship::{Mode, Status, load_status};
use i18n_tool::{generate_all, lint};
use serde_json::{Value, json};

const LANGUAGES: &str = r#"source = "en"
[language.en]
name = "English"
stage = "released"
[language.sv]
name = "Svenska"
stage = "draft"
drafts = "ship"
[language.ja]
name = "日本語"
stage = "draft"
drafts = "ship"
"#;

const LINT: &str = "[all]\nnames = [\"Zorblax\"]\nacronyms = [\"TV\"]\n[language.en]\nwords = [\"provider\"]\n";

const TERMS: &str = "# A place episodes come from.\n-source = source\n-source-cap = Source\n-source-pl = sources\n-source-cap-pl = Sources\n# The product name.\n-app-name = anistream\n";
const SV_TERMS: &str = "-source = källa\n-source-cap = Källa\n-source-pl = källor\n-source-cap-pl = Källor\n-app-name = anistream\n";
const JA_TERMS: &str = "-source = ソース\n-source-cap = ソース\n-source-pl = ソース\n-source-cap-pl = ソース\n-app-name = anistream\n";

const LIBRARY: &str = r#"# Page title.
library-title = Your library

# Cut first when the row is short.
# @arg count: int
# @arg title: text
# @max 40
library-left = { $count ->
    [one] One episode left in { $title }
   *[other] { $count } episodes left in { $title }
 }

# Button.
# @max 16
library-add = Add { -source }
"#;
const SETTINGS: &str = "# Row.\nsettings-language = Language\n";
const LEGAL: &str = "# Legal line.\nlegal-hosts-no-content = { -app-name } hosts no content.\n";

fn put(root: &Path, path: &str, text: &str) {
    let full = root.join("i18n").join(path);
    fs::create_dir_all(full.parent().unwrap()).unwrap();
    fs::write(full, text).unwrap();
}

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let r = dir.path();
    put(r, "languages.toml", LANGUAGES);
    put(r, "lint.toml", LINT);
    put(r, "en/terms.ftl", TERMS);
    put(r, "en/ui-library.ftl", LIBRARY);
    put(r, "en/ui-settings.ftl", SETTINGS);
    put(r, "en/engine.ftl", LEGAL);
    put(r, "sv/terms.ftl", SV_TERMS);
    put(r, "ja/terms.ftl", JA_TERMS);
    dir
}

fn run(root: &Path, tag: &str, group: Option<&str>, proposals: Value) -> Value {
    accept(root, tag, &proposals, group, "test", "2026-11-01").unwrap().to_json()
}

fn reason(report: &Value, id: &str) -> String {
    report["rejected"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap_or_else(|| panic!("{id} was not rejected: {report}"))["reason"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn the_list_has_everything_a_translator_needs_one_batch_per_file() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let out = list(&cat, "sv", None).unwrap();
    assert_eq!(out["plural_categories"], json!(["one", "other"]));
    assert_eq!(out["terms_missing"], 0);
    let groups: Vec<&str> = out["batches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["group"].as_str().unwrap())
        .collect();
    assert_eq!(groups, ["ui-library", "ui-settings"], "the legal line is held back");
    let left = &out["batches"][0]["messages"][1];
    assert_eq!(left["id"], "library-left");
    assert_eq!(left["state"], "missing");
    assert_eq!(left["max"], 40);
    assert_eq!(left["comment"], "Cut first when the row is short.");
    assert_eq!(
        left["args"],
        json!([{"name": "count", "type": "int"}, {"name": "title", "type": "text"}])
    );
    assert_eq!(left["plural"]["selector"], "count");
    assert!(left["english"].as_str().unwrap().contains("[one] One episode left in { $title }"));
    let add = &out["batches"][0]["messages"][2];
    assert_eq!(add["english"], "Add { -source }");
    assert_eq!(add["terms"], json!({"-source": "källa"}));

    let only = list(&cat, "ja", Some("ui-settings")).unwrap();
    assert_eq!(only["plural_categories"], json!(["other"]));
    assert_eq!(only["batches"].as_array().unwrap().len(), 1);
    assert!(list(&cat, "ja", Some("ui-nowhere")).is_err());
    assert!(list(&cat, "en", None).is_err());
}

#[test]
fn accepted_translations_become_drafts_that_ship() {
    let dir = fixture();
    let root = dir.path();
    let report = run(
        root,
        "sv",
        Some("ui-library"),
        json!({
            "library-title": "Ditt bibliotek",
            "library-left": {"one": "Ett avsnitt kvar i { $title }", "other": "{ $count } avsnitt kvar i { $title }"},
            "library-add": "Ny { -source }",
        }),
    );
    assert_eq!(report["accepted"], 3, "{report}");
    assert_eq!(report["left"], 0);
    let file = fs::read_to_string(root.join("i18n/sv/ui-library.ftl")).unwrap();
    assert!(file.contains("library-title = Ditt bibliotek\n"));
    assert!(file.contains("[one] Ett avsnitt kvar i { $title }"));

    let cat = Catalogue::load(root).unwrap();
    let status: Status = load_status(&cat, "sv").unwrap();
    let e = &status["library-title"];
    assert_eq!(
        (e.state.as_str(), e.by.as_str(), e.at.as_str()),
        ("draft", "test", "2026-11-01")
    );
    assert_eq!(e.source, i18n_tool::ship::hash(cat.source().message("library-title").unwrap()));
    // Nothing left to list for the file, and the settings file is still waiting.
    let out = list(&cat, "sv", None).unwrap();
    assert_eq!(out["batches"].as_array().unwrap().len(), 1);
    // Drafts of a language that ships them reach the generated files.
    let files = generate_all(&cat, Mode::Release).unwrap();
    let android = files.iter().find(|(p, _)| p.ends_with("values-sv/strings.xml")).unwrap().1;
    assert!(android.contains("Ditt bibliotek"));
    // The lint is content with what was written.
    assert!(lint::run(&cat).unwrap().iter().all(|f| f.severity != lint::Severity::Error));
}

#[test]
fn a_changed_english_message_comes_back_as_stale_with_the_old_draft() {
    let dir = fixture();
    let root = dir.path();
    run(root, "sv", None, json!({"settings-language": "Språk"}));
    put(root, "en/ui-settings.ftl", "# Row.\nsettings-language = App language\n");
    let cat = Catalogue::load(root).unwrap();
    let out = list(&cat, "sv", Some("ui-settings")).unwrap();
    let m = &out["batches"][0]["messages"][0];
    assert_eq!((m["state"].as_str(), m["previous"].as_str()), (Some("stale"), Some("Språk")));
    // Redrafting replaces the entry in place.
    run(root, "sv", None, json!({"settings-language": "Appspråk"}));
    let text = fs::read_to_string(root.join("i18n/sv/ui-settings.ftl")).unwrap();
    assert_eq!(text, "settings-language = Appspråk\n");
}

#[test]
fn each_check_refuses_with_a_reason() {
    let dir = fixture();
    let root = dir.path();
    let report = run(
        root,
        "sv",
        None,
        json!({
            "library-title": "Ditt { $nope }",
            "library-left": {"other": "{ $count } avsnitt kvar i { $title }"},
            "library-add": "Lägg till en ny källa här",
            "settings-language": "Språk \u{2014} val",
            "no-such-message": "x",
            "legal-hosts-no-content": "anistream lagrar inget.",
        }),
    );
    assert_eq!(report["accepted"], 0, "{report}");
    assert!(reason(&report, "library-title").contains("$nope"));
    assert!(
        reason(&report, "library-left")
            .contains("plural forms [other] but sv needs [one, other]")
    );
    let add = reason(&report, "library-add");
    assert!(add.contains("types \"källa\"") && add.contains("over its @max 16"), "{add}");
    assert!(reason(&report, "settings-language").contains("em dash"));
    assert!(reason(&report, "no-such-message").contains("not a message"));
    assert!(reason(&report, "legal-hosts-no-content").contains("person"));
    assert!(!root.join("i18n/status/sv.json").exists(), "nothing accepted, nothing written");

    let more = run(
        root,
        "sv",
        Some("ui-library"),
        json!({
            "library-title": "Ditt {",
            "settings-language": "Språk",
            "library-add": "Zorblax finns här",
        }),
    );
    assert!(reason(&more, "library-title").contains("does not parse"));
    assert!(reason(&more, "settings-language").contains("belongs to ui-settings"));
    assert!(reason(&more, "library-add").contains("Zorblax"));
    let missing_arg = run(
        root,
        "sv",
        None,
        json!({"library-left": {"one": "Ett avsnitt", "other": "{ $count } avsnitt"}}),
    );
    assert!(reason(&missing_arg, "library-left").contains("leaves out $title"));
}

#[test]
fn a_reviewed_current_message_is_left_alone() {
    let dir = fixture();
    let root = dir.path();
    run(root, "sv", None, json!({"settings-language": "Språk"}));
    let path = root.join("i18n/status/sv.json");
    let text = fs::read_to_string(&path).unwrap().replace("\"draft\"", "\"reviewed\"");
    fs::write(&path, text).unwrap();
    let report = run(root, "sv", None, json!({"settings-language": "Annat"}));
    assert!(reason(&report, "settings-language").contains("already reviewed"));
}

#[test]
fn japanese_has_one_plural_form_and_its_terms_are_checked_inside_unspaced_text() {
    let dir = fixture();
    let root = dir.path();
    let report = run(
        root,
        "ja",
        None,
        json!({
            "library-left": {"other": "{ $title }の残り{ $count }話"},
            "library-add": "ソースを追加",
            "library-title": "ライブラリ",
        }),
    );
    assert_eq!(report["accepted"], 2, "{report}");
    let add = reason(&report, "library-add");
    assert!(add.contains("types \"ソース\""), "{add}");
    let file = fs::read_to_string(root.join("i18n/ja/ui-library.ftl")).unwrap();
    assert!(file.contains("*[other] { $title }の残り{ $count }話"));
    // A katakana word that merely contains the term is not the term.
    let ok = run(root, "ja", None, json!({"library-add": "リソースを追加"}));
    assert_eq!(ok["accepted"], 1, "{ok}");
}

#[test]
fn terms_go_through_the_same_door() {
    let dir = fixture();
    let root = dir.path();
    put(root, "sv/terms.ftl", "-app-name = anistream\n");
    let cat = Catalogue::load(root).unwrap();
    let out = list(&cat, "sv", None).unwrap();
    assert_eq!(out["terms_missing"], 4);
    assert_eq!(out["batches"][0]["group"], "terms");
    assert_eq!(out["batches"][0]["messages"][0]["comment"], "A place episodes come from.");

    let report = run(
        root,
        "sv",
        Some("terms"),
        json!({
            "-source": "källa", "-source-cap": "Källa", "-source-pl": "källor",
            "-source-cap-pl": "Källor — alla", "-nothing": "x",
        }),
    );
    assert_eq!(report["accepted"], 4 - 1, "{report}");
    assert!(reason(&report, "-source-cap-pl").contains("em dash"));
    assert!(reason(&report, "-nothing").contains("not a term"));
    let file = fs::read_to_string(root.join("i18n/sv/terms.ftl")).unwrap();
    assert!(file.contains("# A place episodes come from.\n-source = källa\n"), "{file}");
    assert!(file.contains("-app-name = anistream"));
    assert_eq!(report["left"], 1);
    assert!(
        !root.join("i18n/status/sv.json").exists(),
        "terms have no review state of their own"
    );
}

#[test]
fn a_second_agents_notes_are_stored_with_the_draft_and_reset_by_a_redraft() {
    let dir = fixture();
    let root = dir.path();
    run(root, "sv", None, json!({"settings-language": "Språk"}));
    let cat = Catalogue::load(root).unwrap();
    let rows = i18n_tool::draft::drafts(&cat, "sv", None).unwrap();
    assert_eq!(rows["batches"][0]["messages"][0]["translation"], "Språk");
    let (stored, bad) = i18n_tool::draft::annotate(
        root,
        "sv",
        &json!({"settings-language": {"back": "Language", "flags": ["tone: fine"]}, "nope": {"back": "x"}}),
    )
    .unwrap();
    assert_eq!((stored, bad.len()), (1, 1));
    let status = load_status(&Catalogue::load(root).unwrap(), "sv").unwrap();
    assert_eq!(status["settings-language"].back, "Language");
    assert_eq!(status["settings-language"].flags, ["tone: fine"]);
    run(root, "sv", None, json!({"settings-language": "Appspråk"}));
    let status = load_status(&Catalogue::load(root).unwrap(), "sv").unwrap();
    assert!(
        status["settings-language"].flags.is_empty()
            && status["settings-language"].back.is_empty()
    );
}
