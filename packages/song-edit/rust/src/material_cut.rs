// material_cut.rs — cutting and merging MATERIAL, where track edits move and resize PLACEMENTS: a sequence split in two, a range made one.
// A reader would get wrong what an audio split writes: no new file, two windowed placements of one source (docs/materialCut.md).

use crate::chain_item_edit::{EventsAt, lane_events_at, lane_folder};
use crate::error::{Res, fail};
use crate::fs::Fs;
use crate::material_notes::{
    EDGE_EPS, Edges, Note, WrittenMaterial, free_sequence_name, loops_of, notes_in, notes_of, ref_of, sort_notes,
    window_of, write_sequence,
};
use crate::module_io::entity_file_for;
use crate::paths::join;
use crate::song_tempo::song_tempo;
use crate::start_time::{above, js_round, start_as, start_beats, to_position};
use crate::track_edit::edit_track_struct;
use crate::track_file::{
    add_placement, copy_placement_at, find_placement_at, placement_at, placement_values, remove_placement_at,
    set_placement_field,
};
use crate::workspace_doc::ws_text;
use k7x::edit::{Document, Value};
use std::collections::HashSet;

/// What a physical split wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicalSplitReport {
    pub head: WrittenMaterial,
    pub tail: WrittenMaterial,
    pub from: String,
}

/// What a consolidation wrote.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsolidateReport {
    pub written: WrittenMaterial,
    pub replaced: usize,
    pub from: Vec<String>,
}

fn lane_of(fs: &dyn Fs, dir: &str, track: &str) -> Res<EventsAt> {
    match lane_events_at(fs, dir, track)? {
        Some(own) => Ok(own),
        None => fail(format!("track \"{track}\" has no file in {}/", lane_folder(fs, dir))),
    }
}

/// Cuts a SEQUENCE placement's material in two at `at_beats` and plays the halves.
///
/// # Errors
/// A track with no file, a placement that is not a sequence or repeats, a cut outside the event, or a write's.
pub fn split_sequence_material(
    fs: &mut dyn Fs,
    dir: &str,
    track: &str,
    from_beats: f64,
    at_beats: f64,
    meter: (f64, f64),
) -> Res<Option<PhysicalSplitReport>> {
    let own = lane_of(fs, dir, track)?;
    let tempo = song_tempo(fs, dir)?;
    let mut doc = Document::new(&ws_text(fs, &own.path)?);
    let lane = doc.at(&own.base);
    let Some(index) = find_placement_at(&lane, from_beats, meter.0, meter.1, tempo)? else {
        return Ok(None);
    };
    let placement = placement_values(&lane)?
        .into_iter()
        .nth(index)
        .unwrap_or(Value::Map(vec![]));
    let ref_ = ref_of(&placement);
    let seqs = join(&[dir, "sequences"]);
    let Some(file) = entity_file_for(fs, &seqs, "sequences", &ref_) else {
        return fail(format!(
            "'{ref_}' is not a sequence in this project — an audio split is two windowed placements, splitAudioPlacement"
        ));
    };
    let loops = loops_of(&placement);
    if loops > 1.0 {
        return fail(format!(
            "'{ref_}' repeats {} times here — set its loop back to 1 before splitting it",
            k7x::edit::js_number(loops)
        ));
    }
    let win = window_of(fs, dir, &ref_, &placement, meter)?;
    let cut = win.from + (at_beats - from_beats);
    if !above(at_beats, from_beats + EDGE_EPS) {
        return fail("the cut has to be after the event starts");
    }
    if win.to.is_some_and(|t| cut >= t - EDGE_EPS) {
        return fail("the cut has to be before the event ends");
    }
    let all = notes_of(fs, &file.path);
    let end = win
        .to
        .unwrap_or_else(|| all.iter().fold(win.from, |m, n| m.max(n.t + n.d)).max(cut));
    let mut taken = HashSet::new();
    let name = free_sequence_name(fs, dir, &ref_, &mut taken);
    let edges = Edges {
        from: None,
        to: cut - win.from,
        verb: "event split",
    };
    let head = write_sequence(fs, dir, &name, &notes_in(&all, win.from, cut), Some(&edges), meter)?;
    let name = free_sequence_name(fs, dir, &ref_, &mut taken);
    let edges = Edges {
        from: None,
        to: end - cut,
        verb: "event split",
    };
    let tail = write_sequence(fs, dir, &name, &notes_in(&all, cut, end), Some(&edges), meter)?;

    // ONE write for both halves: a file caught between them would play the head twice.
    edit_track_struct(fs, dir, track, "split an event's material", |doc| {
        let Some(i) = find_placement_at(doc, from_beats, meter.0, meter.1, tempo)? else {
            return Ok(());
        };
        // Every window on the placement is cleared: both halves are whole materials.
        set_placement_field(doc, i, "ref", Some(Value::Str(head.ref_.clone())))?;
        set_placement_field(doc, i, "from", None)?;
        set_placement_field(doc, i, "to", None)?;
        let like = placement_at(doc, i)?;
        let start = start_as(
            like.as_ref().and_then(|p| p.get("start")),
            at_beats,
            meter.0,
            meter.1,
            tempo,
        )?;
        copy_placement_at(
            doc,
            i,
            &[
                ("ref", Some(Value::Str(tail.ref_.clone()))),
                ("start", Some(start)),
                ("from", None),
                ("to", None),
            ],
        )
    })?;
    Ok(Some(PhysicalSplitReport { head, tail, from: ref_ }))
}

/// Writes ONE sequence over `[from_beats, to_beats]`, gaps kept.
///
/// # Errors
/// A track with no file, a placement that is not a sequence, or a write's.
pub fn consolidate_sequence_material(
    fs: &mut dyn Fs,
    dir: &str,
    track: &str,
    from_beats: f64,
    to_beats: f64,
    meter: (f64, f64),
) -> Res<Option<ConsolidateReport>> {
    let own = lane_of(fs, dir, track)?;
    let mut doc = Document::new(&ws_text(fs, &own.path)?);
    let lane = doc.at(&own.base);
    let tempo = song_tempo(fs, dir)?;
    let mut picked: Vec<(Value, f64)> = vec![];
    for p in placement_values(&lane)? {
        if let Ok(start) = start_beats(p.get("start"), meter.0, meter.1, tempo, "1:1:1")
            && start >= from_beats - EDGE_EPS
            && start <= to_beats + EDGE_EPS
        {
            picked.push((p, start));
        }
    }
    picked.sort_by(|a, b| a.1.total_cmp(&b.1));
    if picked.is_empty() {
        return Ok(None);
    }
    let mut notes: Vec<Note> = vec![];
    let mut from = vec![];
    for (p, start) in &picked {
        let ref_ = ref_of(p);
        let Some(file) = entity_file_for(fs, &join(&[dir, "sequences"]), "sequences", &ref_) else {
            return fail(format!(
                "'{ref_}' is not a sequence — consolidating audio renders a new file, which is @studio/commands' half"
            ));
        };
        from.push(ref_.clone());
        let win = window_of(fs, dir, &ref_, p, meter)?;
        let all = notes_of(fs, &file.path);
        let end = win
            .to
            .unwrap_or_else(|| all.iter().fold(win.from, |m, n| m.max(n.t + n.d)));
        let loops = loops_of(p);
        let span = end - win.from;
        // A repeat is UNROLLED rather than refused: N copies of one window laid end to end.
        let mut pass = 0.0;
        while pass < loops {
            for n in notes_in(&all, win.from, end) {
                notes.push(Note {
                    t: n.t + (start - from_beats) + pass * span,
                    ..n
                });
            }
            pass += 1.0;
        }
    }
    sort_notes(&mut notes);
    // The declared window is the RECTANGLE, never narrowed below what it holds.
    let last = notes.iter().fold(0.0_f64, |e, n| e.max(n.t + n.d));
    let mut taken = HashSet::new();
    let name = free_sequence_name(fs, dir, track, &mut taken);
    let edges = Edges {
        from: Some(0.0),
        to: (to_beats - from_beats).max(last),
        verb: "selection consolidate",
    };
    let written = write_sequence(fs, dir, &name, &notes, Some(&edges), meter)?;

    // Highest index first, so removing one does not renumber the next.
    edit_track_struct(fs, dir, track, "consolidate a range", |doc| {
        for (_, start) in picked.iter().rev() {
            if let Some(i) = find_placement_at(doc, *start, meter.0, meter.1, tempo)? {
                remove_placement_at(doc, i)?;
            }
        }
        let placement = Value::Map(vec![
            ("ref".to_owned(), Value::Str(written.ref_.clone())),
            (
                "start".to_owned(),
                Value::Str(to_position(from_beats, meter.0, meter.1)?),
            ),
        ]);
        add_placement(doc, &placement)
    })?;
    Ok(Some(ConsolidateReport {
        written,
        replaced: picked.len(),
        from,
    }))
}

/// Cuts an AUDIO placement in two WITHOUT a render: two placements of one source, windowed either side of the cut.
///
/// # Errors
/// A track with no file, or a write's.
pub fn split_audio_placement(
    fs: &mut dyn Fs,
    dir: &str,
    track: &str,
    from_beats: f64,
    at_beats: f64,
    meter: (f64, f64),
    cut: f64,
) -> Res<Option<f64>> {
    // Four decimals, as a resize writes a window: a split must not leave a hair of float noise in the file.
    let at = js_round(cut * 10000.0) / 10000.0;
    let tempo = song_tempo(fs, dir)?;
    let mut found = false;
    edit_track_struct(fs, dir, track, "split an event", |doc| {
        let Some(i) = find_placement_at(doc, from_beats, meter.0, meter.1, tempo)? else {
            return Ok(());
        };
        found = true;
        // The tail: the same placement from the cut on, keeping the end and the fade-out, losing the fade-in.
        let like = placement_at(doc, i)?;
        let start = start_as(
            like.as_ref().and_then(|p| p.get("start")),
            at_beats,
            meter.0,
            meter.1,
            tempo,
        )?;
        copy_placement_at(
            doc,
            i,
            &[("start", Some(start)), ("from", Some(Value::Num(at))), ("fadeIn", None)],
        )?;
        // The head ends at the cut and loses the fade-out the tail now has.
        set_placement_field(doc, i, "to", Some(Value::Num(at)))?;
        set_placement_field(doc, i, "fadeOut", None)
    })?;
    Ok(found.then_some(at))
}
