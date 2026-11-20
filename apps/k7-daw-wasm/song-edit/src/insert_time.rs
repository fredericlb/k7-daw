// insert_time.rs — insert or delete a span of song time across everything positioned in it: lanes, takes, curves and markers.
// A reader would get wrong when a file is written: only once EVERY edit is made, so a throw midway leaves the song as it was.

use crate::chain_item_edit::{chain_files, edit_chain_item_in, event_item_ids, lane_folder};
use crate::curve_file::splice_curve_time;
use crate::doc_names::docs_in;
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::paths::join;
use crate::project_root::load_project_root;
use crate::start_time::{above, js_string, js_to_number, to_position};
use crate::time_splice::{Mode, TimeSplice};
use crate::track_file::splice_time;
use crate::workspace_doc::{ws_text, ws_write_text};
use k7x::edit::{Document, Scope, Value, WriteOptions};

/// The project's root file, where the markers are.
const PROJECT_FILE: &str = "project.k7x";

/// What a splice touched, by kind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TimeSpliceReport {
    pub files: Vec<String>,
    pub moved: usize,
    pub removed: usize,
}

/// Every edited document by absolute path, in the order first held, with the text it was read as.
struct Pending(Vec<(String, Document, String)>);

impl Pending {
    /// The index of the document held for `path`, read on first use; None when the file is absent.
    fn held(&mut self, fs: &dyn Fs, path: &str, empty: bool) -> Res<Option<usize>> {
        if let Some(i) = self.0.iter().position(|(p, _, _)| p == path) {
            return Ok(Some(i));
        }
        let before = match ws_text(fs, path) {
            Ok(t) => t,
            Err(_) if empty => String::new(),
            Err(_) => return Ok(None),
        };
        let doc = Document::new(&before);
        // Refused whole rather than skipped: a splice that leaves one file behind shifts the song apart.
        if let Some(e) = doc.errors().first() {
            return fail(format!("{path}: {e}"));
        }
        self.0.push((path.to_owned(), doc, before));
        Ok(Some(self.0.len() - 1))
    }
}

/// Edits, holds, and records the file only when its text actually changed.
fn edit(
    fs: &dyn Fs,
    dir: &str,
    rel: &str,
    pending: &mut Pending,
    report: &mut TimeSpliceReport,
    run: impl FnOnce(&mut Document) -> Res<TimeSplice>,
) -> Res<()> {
    let Some(i) = pending.held(fs, &join(&[dir, rel]), false)? else {
        return Ok(());
    };
    let doc = &mut pending.0[i].1;
    let was = doc.text().to_owned();
    let done = run(doc)?;
    if doc.text() == was {
        return Ok(());
    }
    if !report.files.iter().any(|f| f == rel) {
        report.files.push(rel.to_owned());
    }
    report.moved += done.moved;
    report.removed += done.removed;
    Ok(())
}

/// Runs a lane-file writer over every chain item with events, summing what each one moved.
fn each_event_item(doc: &mut Document, run: &dyn Fn(&mut Scope) -> Res<TimeSplice>) -> Res<TimeSplice> {
    let mut sum = TimeSplice::default();
    for id in event_item_ids(doc.text()) {
        edit_chain_item_in(doc, &id, |item| {
            let r = run(item)?;
            sum.moved += r.moved;
            sum.removed += r.removed;
            Ok(())
        })?;
    }
    Ok(sum)
}

/// `parseInt(n, 10)`: the leading integer of a string, NaN when it has none.
fn parse_int10(s: &str) -> f64 {
    let t = s.trim_start_matches(crate::format_level::is_js_space);
    let (sign, rest) = match t.strip_prefix('-') {
        Some(r) => (-1.0, r),
        None => (1.0, t.strip_prefix('+').unwrap_or(t)),
    };
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        f64::NAN
    } else {
        sign * digits.parse::<f64>().unwrap_or(f64::NAN)
    }
}

/// Inserts (or deletes) `len_beats` of time at `at_beats`, everything judged by its `start`.
///
/// # Errors
/// A project root that does not read, a meter that is not `n/d`, a file that does not parse, or a write's.
pub fn splice_project_time(
    fs: &mut dyn Fs,
    dir: &str,
    at_beats: f64,
    len_beats: f64,
    mode: Mode,
) -> Res<TimeSpliceReport> {
    let mut report = TimeSpliceReport::default();
    if !above(len_beats, 0.0) {
        return Ok(report);
    }
    let mut pending = Pending(vec![]);
    let root = load_project_root(fs, dir, true)?;
    let proj = root.project();
    let present = |k: &str| proj.and_then(|p| p.get(k)).filter(|v| !matches!(v, Value::Null));
    let ts = present("timeSignature").map_or("4/4".to_owned(), js_string);
    let mut parts = ts.split('/').map(parse_int10);
    let (num, den) = (parts.next().unwrap_or(f64::NAN), parts.next().unwrap_or(f64::NAN));
    if num.is_nan() || den.is_nan() || num == 0.0 || den == 0.0 {
        return fail(format!("timeSignature must look like '4/4', got '{ts}'"));
    }
    let tempo = present("tempo").map_or(120.0, js_to_number);
    let end = at_beats + len_beats;

    // 1. The LANES: what is playing, every take of it, and every alternative's own two lists.
    let lanes = |doc: &mut Scope| splice_time(doc, at_beats, len_beats, mode, (num, den, tempo));
    let folder = lane_folder(fs, dir);
    for f in docs_in(fs, &join(&[dir, folder])) {
        edit(fs, dir, &format!("{folder}/{f}"), &mut pending, &mut report, |doc| {
            lanes(&mut doc.at(&[]))
        })?;
    }
    // ...and in a chain project, any node item that holds events of its own.
    for rel in chain_files(fs, dir)? {
        edit(fs, dir, &rel, &mut pending, &mut report, |doc| {
            each_event_item(doc, &lanes)
        })?;
    }

    // 2. The CURVES: only a `custom` one is positioned, an LFO being periodic and an ADSR per-trigger.
    for f in docs_in(fs, &join(&[dir, "curves"])) {
        let rel = format!("curves/{f}");
        let run = |doc: &mut Document| splice_curve_time(&mut doc.at(&[]), at_beats, len_beats, mode, num, den);
        edit(fs, dir, &rel, &mut pending, &mut report, run)?;
    }

    // 3. The MARKERS, one map at project.k7x's root, rewritten whole as `project marker new` writes it.
    let markers = root.markers(num, den);
    if !markers.is_empty() {
        let drops = |b: f64| mode == Mode::Delete && b >= at_beats - 1e-6 && b < end - 1e-6;
        let kept: Vec<_> = markers.iter().filter(|m| !drops(m.beat)).collect();
        let from = if mode == Mode::Delete { end } else { at_beats };
        let mut touched = markers.len() - kept.len();
        let mut map: Vec<(String, Value)> = vec![];
        for m in &kept {
            let shift = if mode == Mode::Delete { -len_beats } else { len_beats };
            let shifted = if m.beat >= from - 1e-6 { m.beat + shift } else { m.beat };
            if shifted != m.beat {
                touched += 1;
            }
            map.push((to_position(shifted, num, den)?, Value::Str(m.name.clone())));
        }
        if touched > 0 {
            let at = pending.held(fs, &join(&[dir, PROJECT_FILE]), true)?;
            if let Some(i) = at {
                pending.0[i].1.set_in(
                    &[crate::track_file::key("markers")],
                    &Value::object(map),
                    WriteOptions::default(),
                )?;
            }
            if !report.files.iter().any(|f| f == PROJECT_FILE) {
                report.files.push(PROJECT_FILE.to_owned());
            }
            report.moved += kept.len();
            report.removed += markers.len() - kept.len();
        }
    }

    // Every text is made: only now does any reach the disk, each one whole.
    for (path, doc, before) in &pending.0 {
        if doc.text() != before {
            ws_write_text(fs, path, doc.text())?;
        }
    }
    Ok(report)
}
