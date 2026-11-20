//! jobs_manifest.rs — `parse_manifest` and `move_into` against the TypeScript's answers, and the departures of B2 (2320, DEPARTS 2320b).
//! A reader gets wrong that a vector naming a file outside the staging, a relative path or two files of one name is the TypeScript's
//! answer kept as it was: the native one is the sentence beside it here, and the departures are exactly these three.

use crate::support::jobs::Tmp;
use k7_file_service::jobs::manifest::{OUTSIDE, move_into, parse_manifest};
use serde_json::json;

#[test]
fn the_javascript_trim_and_line_rules_hold() {
    // U+FEFF is white space to JavaScript's trim; U+0085 is not.
    assert_eq!(
        parse_manifest("\u{feff}{\"mix\":\"a\"}\u{feff}").unwrap(),
        json!({ "mix": "a" })
    );
    assert!(parse_manifest("\u{85}{\"mix\":\"a\"}").is_err());
    // A `\r` before a newline goes with the separator.
    assert_eq!(
        parse_manifest("x\r\n{\"mix\":\"a\"}\r\n").unwrap(),
        json!({ "mix": "a" })
    );
}

#[test]
fn a_file_outside_the_staging_or_through_a_link_out_of_it_is_refused_and_nothing_is_copied() {
    let tmp = Tmp::new("jobsmovelink");
    tmp.write("s/mix.wav", "m");
    tmp.write("elsewhere/secret.wav", "secret");
    let (staging, dest) = (tmp.at("s"), tmp.at("dest"));
    let outside = move_into(
        &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("elsewhere/secret.wav") }] }),
        &staging,
        &dest,
    );
    assert_eq!(outside, Err(OUTSIDE.to_owned()));
    assert!(
        !std::path::Path::new(&dest).exists(),
        "the mix was not copied before the refusal"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(tmp.at("elsewhere/secret.wav"), tmp.at("s/link.wav")).unwrap();
        let linked = move_into(
            &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("s/link.wav") }] }),
            &staging,
            &dest,
        );
        assert_eq!(linked, Err(OUTSIDE.to_owned()));
    }
    // Two files of one name are refused whole, mix against stem included.
    tmp.write("s/a/mix.wav", "other");
    let twice = move_into(
        &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("s/a/mix.wav") }] }),
        &staging,
        &dest,
    );
    assert_eq!(
        twice,
        Err("the engine's machine named two files 'mix.wav'".to_owned())
    );
}

#[cfg(unix)]
#[test]
fn a_destination_that_is_a_link_is_replaced_never_followed_on_unix_only() {
    let tmp = Tmp::new("jobsmovedest");
    tmp.write("s/mix.wav", "new");
    tmp.write("elsewhere/target.wav", "kept");
    tmp.mkdir("dest");
    std::os::unix::fs::symlink(tmp.at("elsewhere/target.wav"), tmp.at("dest/mix.wav")).unwrap();
    let moved = move_into(
        &json!({ "mix": tmp.at("s/mix.wav") }),
        &tmp.at("s"),
        &tmp.at("dest"),
    )
    .unwrap();
    assert_eq!(moved.mix, "mix.wav");
    assert_eq!(
        std::fs::read_to_string(tmp.at("elsewhere/target.wav")).unwrap(),
        "kept"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.at("dest/mix.wav")).unwrap(),
        "new"
    );
}

#[test]
fn names_that_differ_only_by_case_or_trailing_dots_and_spaces_are_one_name() {
    let tmp = Tmp::new("jobsmovecase");
    tmp.write("s/mix.wav", "m");
    tmp.write("s/A.wav", "a");
    tmp.write("s/a.wav", "b");
    let (staging, dest) = (tmp.at("s"), tmp.at("dest"));
    for second in ["a.wav", "A.WAV", "a.wav.", "a.wav "] {
        let moved = move_into(
            &json!({ "mix": tmp.at("s/mix.wav"), "stems": [
                { "name": "x", "file": tmp.at("s/A.wav") },
                { "name": "y", "file": format!("{}/{second}", tmp.at("s")) },
            ] }),
            &staging,
            &dest,
        );
        assert_eq!(
            moved,
            Err(format!("the engine's machine named two files '{second}'"))
        );
        assert!(!std::path::Path::new(&dest).exists(), "{second}");
    }
    // Different names stay different.
    let two = move_into(
        &json!({ "mix": tmp.at("s/mix.wav"), "stems": [{ "name": "x", "file": tmp.at("s/A.wav") }] }),
        &staging,
        &dest,
    );
    assert_eq!(two.unwrap().stems, [("x".to_owned(), "A.wav".to_owned())]);
}
