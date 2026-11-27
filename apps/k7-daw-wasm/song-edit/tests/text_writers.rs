// text_writers.rs — the text writers the page reaches as `text.*` ops, pinned by the exact text they leave.
// A reader would get wrong that a refused edit is `Ok(false)` with the text untouched: the page keeps the buffer it had.

use k7x::edit::{Document, Value, parse};
use song_edit::assembly_param::{drop_param, set_param};
use song_edit::chain_data::Chain;
use song_edit::format_chain::format_chain;
use song_edit::library_pins::{Pin, parse_pins, write_pins};
use song_edit::mix_cues::with_markers;
use song_edit::playlist_edit::{append_item, move_item, playlist_stub, remove_item};
use song_edit::sequence_file::{NoteIn, NoteRegion, map_notes_in_region};

/// A writer run over a whole file's text: the new text and what the writer answered.
fn edit<R>(text: &str, f: impl FnOnce(&mut k7x::edit::Scope) -> R) -> (String, R) {
    let mut doc = Document::new(text);
    let out = f(&mut doc.at(&[]));
    (doc.text().to_owned(), out)
}

#[test]
fn a_playlist_stub_parses_and_is_filled_one_line_at_a_time() {
    let stub = playlist_stub("répèt du jeudi").unwrap();
    assert_eq!(stub, "// répèt du jeudi\ntitle(\"répèt du jeudi\")\nitems([])\n");
    let (text, done) = edit("title(\"Set\")\nitems([\n  \"Lithium\",\n])\n", |s| {
        append_item(s, "Hells Bells", None)
    });
    assert!(done.unwrap());
    assert_eq!(
        text,
        "title(\"Set\")\nitems([\n  \"Lithium\",\n  \"Hells Bells\",\n])\n"
    );
    let (text, _) = edit("items([\"Lithium\"])\n", |s| {
        append_item(s, "Nevermind/Lithium", Some("inst"))
    });
    assert!(text.contains("song(\"Nevermind/Lithium\").variant(\"inst\")") || text.contains("variant"));
}

#[test]
fn a_playlist_refuses_what_it_cannot_do_and_writes_nothing() {
    let odd = "items(\"nonsense\")\n";
    assert_eq!(edit(odd, |s| append_item(s, "x", None)), (odd.to_owned(), Ok(false)));
    let set = "items([\n  \"a\",\n  // the second\n  song(\"b\").variant(\"v\"),\n  \"c\",\n])\n";
    assert_eq!(edit(set, |s| remove_item(s, 3.0)).1, Ok(false));
    assert_eq!(edit(set, |s| remove_item(s, -1.0)).1, Ok(false));
    assert_eq!(edit("title(\"x\")\n", |s| remove_item(s, 0.0)).1, Ok(false));
    assert_eq!(edit(set, |s| move_item(s, 0.0, 5.0)).1, Ok(false));
    let (text, done) = edit(set, |s| move_item(s, 1.0, 2.0));
    assert_eq!(done, Ok(true));
    assert!(text.contains("// the second"));
    let items = parse(&text).unwrap();
    let songs: Vec<String> = match items.get("items") {
        Some(Value::List(l)) => l
            .iter()
            .map(|i| match i {
                Value::Str(s) => s.clone(),
                other => match other.get("song") {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                },
            })
            .collect(),
        _ => vec![],
    };
    assert_eq!(songs, ["a", "c", "b"]);
}

const A: &str = "records/Daft Punk/Around the world.flac";
const B: &str = "records/Alain Bashung/La nuit je mens.flac";

fn plain() -> String {
    format!("// where this set drops the needle\ncues([\n  \"{A}\",\n  \"{B}\",\n])\n")
}

fn cues(text: &str, reference: &str, markers: &[Option<f64>]) -> String {
    edit(text, |s| with_markers(s, reference, markers)).0
}

#[test]
fn a_cue_is_written_on_one_line_and_cleared_back_to_a_bare_ref() {
    let cued = cues(&plain(), A, &[Some(1.0), Some(2.0), Some(3.0)]);
    assert!(cued.contains(&format!("  ref(\"{A}\").markers([1, 2, 3]),\n")));
    assert!(cued.contains("// where this set drops the needle"));
    assert_eq!(cues(&cued, A, &[None]), plain());
    // Trailing empties are dropped, a hole is held open.
    let held = cues(&plain(), A, &[None, None, Some(48.0)]);
    assert!(held.contains("markers([null, null, 48])"));
    let trimmed = cues(&plain(), A, &[Some(12.5), None, None, None, None, None, None]);
    assert!(trimmed.contains("markers([12.5])") && !trimmed.contains("null"));
}

#[test]
fn a_cue_replaces_the_songs_entry_and_appends_a_ref_the_sheet_never_named() {
    let twice = cues(&cues(&plain(), A, &[Some(10.0)]), A, &[Some(10.0), Some(20.0)]);
    assert_eq!(twice.matches("Around the world").count(), 1);
    assert!(twice.contains("markers([10, 20])"));
    assert!(cues("cues([])\n", A, &[Some(5.0)]).contains("markers([5])"));
    assert!(cues("", A, &[Some(5.0)]).contains(&format!("ref(\"{A}\").markers([5])")));
    assert_eq!(cues("cues([])\n", A, &[None]), format!("cues([\"{A}\"])\n"));
}

#[test]
fn pins_read_leniently_and_write_in_name_order() {
    let pins = parse_pins("\"k7-core\"(7)\ndrums(\"draft\")\nbass(\"soon\")\nkeys(0)\n");
    assert_eq!(
        pins,
        [
            ("k7-core".to_owned(), Pin::Number(7.0)),
            ("drums".to_owned(), Pin::Draft)
        ]
    );
    let text = write_pins(&pins).unwrap();
    assert_eq!(text, "drums(\"draft\")\nk7-core(7)\n");
    assert_eq!(parse_pins(&text).len(), 2);
    assert!(parse_pins("drums(").is_empty());
}

const TEMPLATE: &str = "// A TURNTABLE — the deck and its strip, as one box.\nlabel(\"Turntable\")\noutputs([\n  // The channel's own fade, which is one half of the crossfader.\n  name(\"mix\").type(\"audio\").node(\"fade\"),\n])\n// The five a hand actually reaches for.\nparams({\n  Trim: { node: \"trim\", param: \"Trim\" },\n})\n";

#[test]
fn a_param_is_promoted_and_taken_back_leaving_the_file_as_it_was() {
    let (two, done) = edit(TEMPLATE, |s| set_param(s, "Level", "level", "Gain"));
    assert!(done.is_ok());
    assert!(two.contains("// The five a hand actually reaches for.") && two.contains("Level"));
    let (back, dropped) = edit(&two, |s| drop_param(s, "Level"));
    assert_eq!(dropped, Ok(true));
    assert_eq!(back, TEMPLATE);
    let (last, _) = edit(TEMPLATE, |s| drop_param(s, "Trim"));
    assert!(!last.contains("params"));
    assert_eq!(
        edit(TEMPLATE, |s| drop_param(s, "Nope")),
        (TEMPLATE.to_owned(), Ok(false))
    );
}

#[test]
fn a_template_that_does_not_read_is_refused_before_any_write() {
    let (text, done) = edit("label(\"x\")\nplugs([])\n", |s| set_param(s, "L", "n", "p"));
    assert!(done.unwrap_err().to_string().starts_with("assembly.k7x: "));
    assert_eq!(text, "label(\"x\")\nplugs([])\n");
}

const NOTES: &str = "from(\"1:1\")\nnotes([\n  t(0).d(1).k(60),\n  t(1).d(0.5).k(64).v(0.4),\n  t(1).d(0.5).k(67).repeat(2),\n  t(4).d(2).k(72),\n])\n";
const ALL: NoteRegion = NoteRegion {
    lo_key: 0.0,
    hi_key: 127.0,
    from: 0.0,
    to: 8.0,
};

fn map(region: &NoteRegion, f: &dyn Fn(&NoteIn) -> Option<NoteIn>) -> (String, Result<usize, String>) {
    let mut doc = Document::new(NOTES);
    let r = map_notes_in_region(&mut doc, region, f).map_err(|e| e.to_string());
    (doc.text().to_owned(), r)
}

fn keys(text: &str) -> Vec<f64> {
    match parse(text).unwrap().get("notes") {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|n| match n.get("k") {
                Some(Value::Num(k)) => Some(*k),
                _ => None,
            })
            .collect(),
        _ => vec![],
    }
}

#[test]
fn a_region_holds_the_notes_that_start_inside_it_both_keys_inclusive() {
    let up = |n: &NoteIn| {
        Some(NoteIn {
            k: n.k + 1.0,
            ..n.clone()
        })
    };
    let (text, got) = map(
        &NoteRegion {
            lo_key: 0.0,
            hi_key: 127.0,
            from: 4.0,
            to: 5.0,
        },
        &up,
    );
    assert_eq!(got, Ok(1));
    assert_eq!(keys(&text)[3], 73.0);
    // A note that merely runs through the box is not held.
    assert_eq!(
        map(
            &NoteRegion {
                lo_key: 0.0,
                hi_key: 127.0,
                from: 5.0,
                to: 6.0
            },
            &up
        )
        .1,
        Ok(0)
    );
    let by_octave = |n: &NoteIn| {
        Some(NoteIn {
            k: n.k + 12.0,
            ..n.clone()
        })
    };
    let (text, got) = map(
        &NoteRegion {
            lo_key: 64.0,
            hi_key: 67.0,
            from: 0.0,
            to: 8.0,
        },
        &by_octave,
    );
    assert_eq!(got, Ok(2));
    assert_eq!(keys(&text), [60.0, 76.0, 79.0, 72.0]);
}

#[test]
fn a_region_holding_nothing_leaves_the_text_and_reports_zero() {
    let nothing = NoteRegion {
        lo_key: 0.0,
        hi_key: 40.0,
        from: 0.0,
        to: 8.0,
    };
    assert_eq!(map(&nothing, &|_| None), (NOTES.to_owned(), Ok(0)));
}

#[test]
fn a_region_rewrite_keeps_what_it_did_not_touch_byte_for_byte() {
    let one = NoteRegion {
        lo_key: 60.0,
        hi_key: 60.0,
        from: 0.0,
        to: 8.0,
    };
    let (text, _) = map(&one, &|n| {
        Some(NoteIn {
            t: n.t + 8.0,
            ..n.clone()
        })
    });
    assert!(text.contains("  t(1).d(0.5).k(64).v(0.4),\n") && text.contains("  t(1).d(0.5).k(67).repeat(2),\n"));
    assert!(text.starts_with("from(\"1:1\")\n"));
    // A pad's effects stay while its pitch changes; the velocity key is cleared, never defaulted back.
    let (text, _) = map(&ALL, &|n| {
        Some(NoteIn {
            k: n.k + 1.0,
            ..n.clone()
        })
    });
    assert!(text.contains("t(1).d(0.5).k(68).repeat(2)"));
    let (text, _) = map(&ALL, &|n| Some(NoteIn { v: None, ..n.clone() }));
    assert!(!text.contains("v("));
    let (text, _) = map(&ALL, &|n| {
        Some(NoteIn {
            v: Some(0.8),
            ..n.clone()
        })
    });
    assert_eq!(text.matches(".v(0.8)").count(), 4);
}

#[test]
fn a_region_removes_from_the_back_and_refuses_a_note_nothing_can_play() {
    // Every note the box holds is reached; the ones the rewrite answers with a note stay.
    let (text, got) = map(&ALL, &|n| (n.k == 72.0).then(|| n.clone()));
    assert_eq!(got, Ok(4));
    assert_eq!(keys(&text), [72.0]);
    let (_, got) = map(
        &NoteRegion {
            lo_key: 0.0,
            hi_key: 127.0,
            from: 0.0,
            to: 2.0,
        },
        &|_| None,
    );
    assert_eq!(got, Ok(3));
    let silent = map(&ALL, &|n| Some(NoteIn { d: 0.0, ..n.clone() })).1;
    assert!(silent.unwrap_err().contains("nothing can play"));
    let beyond = map(&ALL, &|n| Some(NoteIn { k: 200.0, ..n.clone() })).1;
    assert!(beyond.unwrap_err().contains("nothing can play"));
}

fn read_chain(text: &str) -> Chain {
    Chain::from_value(&parse(text).unwrap())
}

const CHAIN: &str = "// the guitar chain\nstart(true)\nsteps([\n  id(\"in\").extends(\"@k7-core/io/in\"),\n  // the drive, set by ear\n  id(\"drive\").extends(\"@k7-core/audio/overdrive\").params({ Amount: 5 }),\n  id(\"verb\").extends(\"@k7-core/audio/reverb\"),\n  to([chain(\"OUT\")]),\n])\n";

#[test]
fn a_chain_is_written_back_as_it_was_and_a_moved_item_keeps_its_comment() {
    let chain = read_chain(CHAIN);
    assert_eq!(format_chain(&chain, CHAIN).unwrap(), CHAIN);
    let mut moved = chain.clone();
    moved.steps.swap(1, 2);
    let text = format_chain(&moved, CHAIN).unwrap();
    assert!(text.contains("// the guitar chain"));
    let verb = text.find("id(\"verb\")").unwrap();
    let note = text.find("// the drive, set by ear").unwrap();
    let drive = text.find("id(\"drive\")").unwrap();
    assert!(verb < note && note < drive);
    let fresh = format_chain(&chain, "").unwrap();
    assert!(!fresh.starts_with("//"));
    assert_eq!(read_chain(&fresh), chain);
}
