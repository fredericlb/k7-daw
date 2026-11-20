//! The rig (1511): `normalizePreferences`' channels and cue pair replayed from node's answers, the file's states, and hostile files.

use std::time::{Duration, Instant};

use crate::support::Scratch;
use k7_file_service::rig::{empty_rig, rig, rig_of_text};
use k7_file_service::space::Declined;
use serde_json::Value;

const BOUND: Duration = Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 30 });

fn vectors() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/rig.json"
    ))
    .unwrap();
    serde_json::from_str(&text).unwrap()
}

fn prefs(s: &Scratch, text: &str) -> Result<Value, Declined> {
    s.file("studio/preferences.k7x", text);
    rig(&s.home())
}

#[test]
fn replays_every_vector_of_normalize_preferences() {
    let all = vectors();
    assert!(all.len() >= 80, "{} cases", all.len());
    for c in &all {
        let text = c["text"].as_str().unwrap();
        let got = rig_of_text(text).unwrap_or_else(|d| panic!("{text:?} declined: {d:?}"));
        // Text, not value: the key order and the number spelling are what the session's key hashes.
        assert_eq!(got.to_string(), c["out"].to_string(), "{text:?}");
    }
}

#[test]
fn the_empty_rig_is_the_defaults_shape_and_not_an_empty_object() {
    assert_eq!(
        empty_rig().to_string(),
        r#"{"channels":{"in":[],"out":[]}}"#
    );
    let s = Scratch::new("rig-none");
    assert_eq!(rig(&s.home()).unwrap().to_string(), empty_rig().to_string());
}

#[test]
fn a_preferences_file_is_read_and_a_text_that_is_not_a_mapping_is_the_empty_rig() {
    let s = Scratch::new("rig-read");
    let got = prefs(
        &s,
        "channels({ in: [ch([1]).label(\"gtr\")], out: [] })\ncueOutput(\"mon\")\n",
    )
    .unwrap();
    assert_eq!(
        got.to_string(),
        r#"{"channels":{"in":[{"ch":[1],"label":"gtr"}],"out":[]},"cueOutput":"mon"}"#
    );
    for text in ["this does not parse (", "[1, 2]", "null", "5", "a(1)\na(2)"] {
        assert_eq!(prefs(&s, text).unwrap(), empty_rig(), "{text:?}");
    }
}

#[test]
fn an_empty_file_alone_a_folder_and_a_link_to_nothing_are_the_empty_rig() {
    let s = Scratch::new("rig-states");
    assert_eq!(prefs(&s, "").unwrap(), empty_rig());
    std::fs::remove_file(s.path("studio/preferences.k7x")).unwrap();
    s.dir("studio/preferences.k7x");
    assert_eq!(rig(&s.home()).unwrap(), empty_rig());
    std::fs::remove_dir(s.path("studio/preferences.k7x")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(s.path("nowhere"), s.path("studio/preferences.k7x")).unwrap();
        // A link to nothing is the empty rig.
        assert_eq!(rig(&s.home()).unwrap(), empty_rig());
    }
}

#[test]
fn a_sixteen_megabyte_file_and_ten_thousand_labels_are_answered_in_time() {
    let s = Scratch::new("rig-big");
    let t = Instant::now();
    let comments =
        "# a line of nothing at all, repeated to fill the file\n".repeat((16 << 20) / 53);
    assert!(prefs(&s, &comments).is_ok());
    let entries: Vec<String> = (0..10_000)
        .map(|i| format!("{{ ch: {}, label: \"l{i}\" }}", i + 1))
        .collect();
    let got = prefs(
        &s,
        &format!("channels({{ in: [{}], out: [] }})\n", entries.join(", ")),
    )
    .unwrap();
    assert_eq!(got["channels"]["in"].as_array().unwrap().len(), 10_000);
    assert!(t.elapsed() < BOUND, "{:?}", t.elapsed());
}

#[test]
fn a_file_past_the_limit_is_declined_and_never_read() {
    let s = Scratch::new("rig-huge");
    s.file("studio/preferences.k7x", "");
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(s.path("studio/preferences.k7x"))
        .unwrap();
    f.set_len(65 << 20).unwrap();
    assert_eq!(rig(&s.home()), Err(Declined::TooLarge));
}

#[test]
fn a_lone_surrogate_is_declined_for_the_typescript() {
    let s = Scratch::new("rig-lone");
    assert_eq!(
        prefs(&s, "cueOutput(\"\\ud800\")\n"),
        Err(Declined::LossyText)
    );
}
