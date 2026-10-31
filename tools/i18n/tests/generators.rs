//! The generators against a small catalogue with every case that has bitten a translation system:
//! a plural with an argument, two arguments in either order, a percent sign, an apostrophe, a
//! leading `@`, braces, a platform-only message, and languages in each review state. The outputs
//! are compared with files in tests/golden/; run with BLESS=1 to rewrite them after reading the
//! diff.

use std::fs;
use std::path::{Path, PathBuf};

use i18n_tool::catalogue::Catalogue;
use i18n_tool::ship::{Entry, Mode, Status, hash};
use i18n_tool::{generate_all, stale, write};

const LANGUAGES: &str = r#"source = "en"
[language.en]
name = "English"
stage = "released"
[language.pl]
name = "Polski"
stage = "beta"
[language.da]
name = "Dansk"
stage = "beta"
drafts = "ship"
"#;

const TERMS: &str = "-app-name = anistream\n-source = source\n-source-cap-pl = Sources\n";

const EN: &str = r#"# Settings row.
settings-language = Language

# Row subtitle on the continue shelf.
# @arg count: int
# @arg title: text
# @max 60
queue-left = { $count ->
    [one] One episode left in { $title }
   *[other] { $count } episodes left in { $title }
 }

# The reasons a match was made.
# @arg percent: percent
match-remembered = Remembered ({ $percent }) at 100%

# Quote and apostrophe handling.
# @arg name: text
quote = Couldn't find "{ $name }" in <{ -source-cap-pl }> & more.

# A handle at the start of a string.
at-home = @home is { -app-name }

# Literal braces.
braces = Use { "{" }name{ "}" } here

# Order changes per language.
# @arg who: text
# @arg what: text
two-args = { $who } watched { $what }
"#;

const ENGINE_EN: &str = r#"# Toast after a setting is stored.
# @arg label: text
notice-saved = Saved { $label }.

# Toast when a setting could not be stored.
# @arg cause: cause
notice-not-saved = Couldn't save that setting. { $cause }

# Toast with a wait.
# @arg wait: seconds
# @arg size: bytes
# @arg count: int
# @arg percent: percent
notice-everything = { $count ->
    [one] One thing
   *[other] { $count } things
 } in { $wait }, { $size }, { $percent }%.

# The storage is full.
cause-disk-full = There is no space left. Free some and try again.

# The file could not be written.
# @arg path: text
cause-not-writable = { $path } is not writable.

# Nothing to say.
notice-done = Done.
"#;

const ANDROID_EN: &str = "# Name of the downloads channel.\nchannel-downloads = Downloads\n";

const PL_TERMS: &str = "-app-name = anistream\n-source = źródło\n-source-cap-pl = Źródła\n";
const PL: &str = r#"settings-language = Język
queue-left = { $count ->
    [one] Jeden odcinek w { $title }
    [few] { $count } odcinki w { $title }
    [many] { $count } odcinków w { $title }
   *[other] { $count } odcinka w { $title }
 }
at-home = @dom to { -app-name }
two-args = { $what } obejrzał { $who }
"#;
const DA: &str = "settings-language = Sprog\nqueue-left = { $count ->\n    [one] Et afsnit tilbage i { $title }\n   *[other] { $count } afsnit tilbage i { $title }\n }\n";

fn entry(source: &str, state: &str) -> Entry {
    Entry {
        source: source.to_owned(),
        state: state.to_owned(),
        by: "agent".into(),
        at: "2026-11-03".into(),
    }
}

/// Writes the fixture; returns the root.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let put = |path: &str, text: &str| {
        let full = root.join("i18n").join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    };
    put("languages.toml", LANGUAGES);
    put("en/terms.ftl", TERMS);
    put("en/ui.ftl", EN);
    put("en/engine.ftl", ENGINE_EN);
    put("en/platform-android.ftl", ANDROID_EN);
    put("pl/terms.ftl", PL_TERMS);
    put("pl/ui.ftl", PL);
    put("da/terms.ftl", TERMS);
    put("da/ui.ftl", DA);

    let cat = Catalogue::load(root).unwrap();
    let h = |id: &str| hash(cat.source().message(id).unwrap());
    // pl: one reviewed, one draft (held back), one reviewed against an older English (stale).
    let mut pl = Status::new();
    pl.insert("settings-language".into(), entry(&h("settings-language"), "reviewed"));
    pl.insert("queue-left".into(), entry(&h("queue-left"), "draft"));
    pl.insert("at-home".into(), entry("000000000000", "reviewed"));
    pl.insert("two-args".into(), entry(&h("two-args"), "reviewed"));
    // da ships its drafts, labelled.
    let mut da = Status::new();
    da.insert("settings-language".into(), entry(&h("settings-language"), "draft"));
    da.insert("queue-left".into(), entry(&h("queue-left"), "draft"));
    for (tag, status) in [("pl", pl), ("da", da)] {
        let path = root.join("i18n/status").join(format!("{tag}.json"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_string_pretty(&status).unwrap()).unwrap();
    }
    dir
}

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

#[test]
fn every_target_matches_its_golden_files() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Release).unwrap();
    let bless = std::env::var_os("BLESS").is_some();
    if bless {
        let _ = fs::remove_dir_all(golden_dir());
    }
    for (path, text) in &files {
        let golden = golden_dir().join(path);
        if bless {
            fs::create_dir_all(golden.parent().unwrap()).unwrap();
            fs::write(&golden, text).unwrap();
        } else {
            let want = fs::read_to_string(&golden)
                .unwrap_or_else(|_| panic!("no golden for {path}; run with BLESS=1"));
            assert_eq!(&want, text, "{path} differs from its golden file");
        }
    }
}

fn file<'a>(files: &'a i18n_tool::util::Files, suffix: &str) -> &'a str {
    files
        .iter()
        .find(|(p, _)| p.ends_with(suffix))
        .unwrap_or_else(|| panic!("no file ending {suffix}"))
        .1
}

#[test]
fn only_shipped_translations_reach_a_catalogue() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Release).unwrap();
    let apple = file(&files, "Localizable.xcstrings");
    assert!(
        !apple.contains("needs_review"),
        "Xcode ships needs_review strings, so none may be written"
    );
    // Polish: the reviewed message is there; the draft and the stale one are not.
    assert!(apple.contains("Język"));
    assert!(!apple.contains("Jeden odcinek"), "a draft in a language that holds drafts");
    assert!(!apple.contains("@dom"), "a translation reviewed against older English");
    // Danish ships drafts, so they are there, and the language is labelled as a draft.
    assert!(apple.contains("Et afsnit tilbage"));
    let swift = file(&files, "L.swift");
    assert!(swift.contains("Language(tag: \"da\", name: \"Dansk\", draft: true)"));
    assert!(swift.contains("Language(tag: \"pl\", name: \"Polski\", draft: true)"));
    // The same filter applies on every platform.
    for suffix in ["values-pl/strings.xml", "pl/Resources.resw", "ftl/pl.ftl"] {
        let text = file(&files, suffix);
        assert!(text.contains("Język"), "{suffix}");
        assert!(!text.contains("Jeden odcinek"), "{suffix}");
    }
    assert!(file(&files, "values-da/strings.xml").contains("Et afsnit tilbage"));
}

#[test]
fn dev_mode_adds_the_drafts() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Dev).unwrap();
    assert!(file(&files, "Localizable.xcstrings").contains("Jeden odcinek"));
    assert!(
        !file(&files, "Localizable.xcstrings").contains("@dom"),
        "stale stays out in dev mode too"
    );
}

#[test]
fn plural_forms_and_arguments_come_out_right() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Dev).unwrap();
    // Android: Polish has four quantities, and the quantity argument stays %1$d.
    let pl = file(&files, "values-pl/strings.xml");
    for q in ["one", "few", "many", "other"] {
        assert!(pl.contains(&format!("<item quantity=\"{q}\">")), "{q}");
    }
    // Word order changes with positional arguments.
    assert!(pl.contains("%2$s obejrzał %1$s"));
    // Windows: the plural keys, and the table picks Polish forms.
    let resw = file(&files, "pl/Resources.resw");
    assert!(resw.contains("QueueLeft_few") && resw.contains("QueueLeft_many"));
    let class = file(&files, "Strings.g.cs");
    assert!(class.contains("public static string QueueLeft(long count, string title)"));
    // Apple: a plural with a text argument after the count.
    assert!(file(&files, "Localizable.xcstrings").contains("%1$lld episodes left in %2$@"));
    // Rust: the typed function takes an i64 and a &str.
    assert!(
        file(&files, "src/t.rs")
            .contains("pub fn queue_left(count: i64, title: &str) -> String")
    );
}

#[test]
fn escaping_survives_each_format() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Release).unwrap();
    let android = file(&files, "res/values/strings.xml");
    assert!(
        android.contains("Couldn\\'t find \\\"%1$s\\\" in &lt;Sources&gt; &amp; more."),
        "{android}"
    );
    assert!(android.contains("Remembered (%1$s) at 100%%"), "{android}");
    assert!(android.contains(">\\@home is anistream<"), "a leading @ is escaped");
    let resw = file(&files, "en/Resources.resw");
    assert!(resw.contains("&lt;Sources&gt; &amp; more."));
    assert!(resw.contains("Use {{name}} here"));
    let apple = file(&files, "Localizable.xcstrings");
    assert!(
        apple.contains("at 100%%"),
        "a literal percent in a message with arguments is doubled"
    );
    // Platform-only messages reach only their platform.
    assert!(android.contains("channel_downloads"));
    assert!(!apple.contains("channel-downloads"));
    assert!(!file(&files, "ftl/en.ftl").contains("channel-downloads"));
}

#[test]
fn check_finds_what_generate_has_not_written() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Release).unwrap();
    assert!(!stale(dir.path(), &files).is_empty(), "nothing is written yet");
    write(dir.path(), &files).unwrap();
    assert_eq!(stale(dir.path(), &files), Vec::<String>::new());

    // A hand edit to a generated file is caught.
    let l = dir.path().join("apple/Packages/AnistreamStrings/Sources/AnistreamStrings/L.swift");
    fs::write(&l, format!("{}// edited\n", fs::read_to_string(&l).unwrap())).unwrap();
    assert!(stale(dir.path(), &files).iter().any(|p| p.contains("L.swift: differs")));
    write(dir.path(), &files).unwrap();

    // A language removed from the catalogue leaves files behind that check reports and generate removes.
    let leftover = dir.path().join("android/i18n/src/main/res/values-fr/strings.xml");
    fs::create_dir_all(leftover.parent().unwrap()).unwrap();
    fs::write(&leftover, "<resources/>").unwrap();
    assert!(
        stale(dir.path(), &files)
            .iter()
            .any(|p| p.contains("values-fr") && p.contains("no longer generated"))
    );
    write(dir.path(), &files).unwrap();
    assert!(!leftover.exists());
    assert!(stale(dir.path(), &files).is_empty());
}

#[test]
fn the_pseudo_locale_is_generated_from_english_and_longer() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Release).unwrap();
    let xa = file(&files, "ftl/en-XA.ftl");
    assert!(xa.contains("settings-language = [Ļàñĝüàĝé~~~]"), "{xa}");
    // Arguments stay intact.
    assert!(xa.contains("{ $title }") && xa.contains("{ $count }"));
}

#[test]
fn every_xml_output_parses_strictly_and_a_resw_is_a_resw() {
    let dir = fixture();
    let cat = Catalogue::load(dir.path()).unwrap();
    let files = generate_all(&cat, Mode::Dev).unwrap();
    let mut xml = 0;
    for (path, text) in &files {
        if !(path.ends_with(".xml") || path.ends_with(".resw")) {
            continue;
        }
        xml += 1;
        assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n"), "{path}");
        let doc = roxmltree::Document::parse(text)
            .unwrap_or_else(|e| panic!("{path} is not well-formed XML: {e}"));
        if path.ends_with(".resw") {
            let root = doc.root_element();
            assert_eq!(root.tag_name().name(), "root", "{path}");
            for name in ["resmimetype", "version", "reader", "writer"] {
                assert!(
                    root.children()
                        .any(|n| n.has_tag_name("resheader")
                            && n.attribute("name") == Some(name)),
                    "{path} lacks the {name} resheader"
                );
            }
            assert!(
                root.children().filter(|n| n.has_tag_name("data")).count() > 0,
                "{path} has no strings"
            );
        }
    }
    assert!(xml >= 4, "expected strings.xml, locales_config.xml and .resw files, saw {xml}");
}

#[test]
fn script_and_region_tags_land_where_each_platform_wants_them() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let put = |path: &str, text: &str| {
        let full = root.join("i18n").join(path);
        fs::create_dir_all(full.parent().unwrap()).unwrap();
        fs::write(full, text).unwrap();
    };
    let tags = [
        ("zh-Hans", "简体中文", "语言"),
        ("zh-Hant", "繁體中文", "語言"),
        ("es-419", "Español (Latinoamérica)", "Idioma"),
        ("pt-BR", "Português (Brasil)", "Idioma"),
        ("ru", "Русский", "Язык"),
    ];
    let mut langs = String::from(
        "source = \"en\"\n[language.en]\nname = \"English\"\nstage = \"released\"\n",
    );
    for (tag, name, _) in tags {
        langs.push_str(&format!(
            "[language.{tag}]\nname = \"{name}\"\nstage = \"draft\"\ndrafts = \"ship\"\n"
        ));
    }
    put("languages.toml", &langs);
    put("en/terms.ftl", "-app-name = anistream\n");
    put(
        "en/ui.ftl",
        "# @arg count: int\nleft = { $count ->\n    [one] One left\n   *[other] { $count } left\n }\nlanguage = Language\n",
    );
    for (tag, _, word) in tags {
        put(&format!("{tag}/terms.ftl"), "-app-name = anistream\n");
        let left = if tag == "ru" {
            "left = { $count ->\n    [one] { $count } остался\n    [few] { $count } осталось\n    [many] { $count } осталось\n   *[other] { $count } осталось\n }\n"
        } else if tag.starts_with("zh") {
            "left = { $count ->\n   *[other] 还剩 { $count }\n }\n"
        } else {
            "left = { $count ->\n    [one] Queda { $count }\n    [many] Quedan { $count }\n   *[other] Quedan { $count }\n }\n"
        };
        put(&format!("{tag}/ui.ftl"), &format!("{left}language = {word}\n"));
    }
    let cat = Catalogue::load(root).unwrap();
    let mut status = Status::new();
    for m in &cat.source().messages {
        status.insert(m.id.clone(), entry(&hash(m), "draft"));
    }
    for (tag, _, _) in tags {
        let path = root.join("i18n/status").join(format!("{tag}.json"));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, serde_json::to_string(&status).unwrap()).unwrap();
    }
    let files = generate_all(&cat, Mode::Release).unwrap();
    let paths: Vec<&String> = files.keys().collect();
    for want in [
        "values-b+zh+Hans/strings.xml",
        "values-b+zh+Hant/strings.xml",
        "values-b+es+419/strings.xml",
        "values-b+pt+BR/strings.xml",
        "values-ru/strings.xml",
        "Strings/zh-Hans/Resources.resw",
        "Strings/es-419/Resources.resw",
        "ftl/zh-Hans.ftl",
        "ftl/pt-BR.ftl",
    ] {
        assert!(paths.iter().any(|p| p.ends_with(want)), "no {want} in {paths:#?}");
    }
    // Android's locale list and Apple's catalogue keep the BCP 47 tags as they are.
    let config = file(&files, "locales_config.xml");
    for tag in ["zh-Hans", "zh-Hant", "es-419", "pt-BR"] {
        assert!(config.contains(&format!("android:name=\"{tag}\"")), "{tag}");
    }
    let apple = file(&files, "Localizable.xcstrings");
    for (tag, _, word) in tags {
        assert!(apple.contains(&format!("\"{tag}\"")), "{tag} key in the xcstrings");
        assert!(apple.contains(word), "{word}");
    }
    // Plural categories follow the language, not the region or script.
    let strings = file(&files, "values-ru/strings.xml");
    assert!(strings.contains("quantity=\"few\"") && strings.contains("quantity=\"many\""));
    assert!(!file(&files, "values-b+zh+Hans/strings.xml").contains("quantity=\"one\""));
    // Windows picks the plural table by the language part only, once for the two Chinese scripts.
    let windows = file(&files, "Strings.g.cs");
    assert_eq!(windows.matches("case \"zh\":").count(), 1);
    assert!(windows.contains("case \"es\":") && windows.contains("case \"pt\":"));
}
