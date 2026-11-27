// set_list.rs — a playlist resolved against the workspace's audio files, and the scaffold's copy of the controls held to the original.
// A reader would get wrong whose cases these are: setList.test.ts and mapping.test.ts; the browser's sort and search stay TypeScript.

use std::collections::HashSet;
use turntables::bindings::{Mapping, required_controls, unmapped};
use turntables::controls::mix_controls;
use turntables::set_list::{SetIssue, resolve_set};
use turntables::vocab::DECKS;

fn files() -> Vec<String> {
    [
        "records/lemo/danse.flac",
        "records/lemo/stems/drums.wav",
        "records/lemo/stems/bass.wav",
        "records/lemo/stems/other.wav",
        "records/lemo/stems/vocals.wav",
        "records/aube.wav",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn playlist(refs: &[&str]) -> String {
    let items: Vec<String> = refs.iter().map(|r| format!("{r:?}")).collect();
    format!("title(\"Set\")\nitems([{}])\n", items.join(", "))
}

#[test]
fn a_line_is_paired_with_its_file_and_its_stems() {
    let set = resolve_set(
        Some(&playlist(&["records/lemo/danse.flac", "records/aube.wav"])),
        Some(&files()),
    );
    let rows: Vec<_> = set
        .entries
        .iter()
        .map(|e| (e.id.as_str(), e.file.is_some(), e.stems))
        .collect();
    assert_eq!(
        rows,
        [
            ("records/lemo/danse.flac", true, true),
            ("records/aube.wav", true, false)
        ]
    );
    let (a, b) = (&set.entries[0], &set.entries[1]);
    assert_eq!((a.name.as_str(), a.artist.as_deref()), ("danse", Some("lemo")));
    assert_eq!((b.name.as_str(), b.artist.as_deref()), ("aube", Some("records")));
}

#[test]
fn a_line_with_no_file_is_kept_and_plays_nothing() {
    let gone = resolve_set(Some(&playlist(&["records/gone.flac"])), Some(&files()));
    assert_eq!(
        (gone.entries[0].file.clone(), gone.entries[0].name.as_str()),
        (None, "gone")
    );
    let outside = resolve_set(Some(&playlist(&["../x.wav"])), Some(&files()));
    assert_eq!(outside.entries[0].file, None);
    // While the workspace's files are not read, nothing is there.
    assert_eq!(
        resolve_set(Some(&playlist(&["records/aube.wav"])), None).entries[0].file,
        None
    );
}

#[test]
fn a_record_named_twice_is_listed_once_slashes_forgiven() {
    let set = resolve_set(
        Some(&playlist(&["records/aube.wav", "/records/aube.wav/"])),
        Some(&files()),
    );
    assert_eq!(set.entries.len(), 1);
    assert_eq!(set.entries[0].name_ref, "records/aube.wav");
}

#[test]
fn a_playlist_that_is_missing_unreadable_or_empty_says_which() {
    let missing = resolve_set(None, Some(&files()));
    assert_eq!(
        (missing.issues.as_slice(), missing.absent),
        (&[SetIssue::NotFound][..], false)
    );
    let broken = resolve_set(Some("items([\n"), Some(&files()));
    assert_eq!(
        (broken.entries.len(), broken.issues.as_slice()),
        (0, &[SetIssue::Unreadable][..])
    );
    let empty = resolve_set(Some("title(\"x\")\nitems([])\n"), Some(&files()));
    assert!(empty.entries.is_empty() && empty.issues.is_empty() && !empty.absent);
}

#[test]
fn the_scaffolds_copy_of_the_controls_is_held_to_the_original() {
    let skeleton = verbs::surface_skeleton("turntables").unwrap();
    let offered: HashSet<&str> = skeleton.iter().map(|(id, _)| id.as_str()).collect();
    let all = mix_controls();
    // Every required control is offered, and nothing is offered that the desk would reject.
    for c in required_controls() {
        assert!(offered.contains(c.id.as_str()), "{} is not scaffolded", c.id);
    }
    for (id, note) in &skeleton {
        let control = all
            .iter()
            .find(|c| &c.id == id)
            .unwrap_or_else(|| panic!("{id} is not a control"));
        assert!(
            note.contains(control.wants.as_deref().unwrap_or("\u{0}")),
            "{id}'s note does not say what it wants"
        );
    }
    for d in DECKS {
        for s in ["stemDrums", "stemBass", "stemOther", "stemVocals"] {
            assert!(offered.contains(format!("{d}.{s}").as_str()));
        }
        // `transport` and `sync` drive no param: offering one prints an error nobody can clear.
        assert!(!offered.contains(format!("{d}.sync").as_str()));
    }
    assert!(!offered.contains("transport"));
    // The scaffolded mapping is all nulls, and the gate reports every required control as unwired.
    let nulls: Mapping = skeleton.iter().map(|(id, _)| (id.clone(), None)).collect();
    assert_eq!(unmapped(&nulls).len(), required_controls().len());
}
