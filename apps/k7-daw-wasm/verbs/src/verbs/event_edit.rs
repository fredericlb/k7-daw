// event_edit.rs — reshaping one event: resize and split, after verbs/eventEdit.ts; trim and consolidate are event_render.rs's.
// A reader would get wrong what an audio split writes: two placements of ONE file, windowed either side; nothing is rendered.

use super::shared::{Done, Found, label, resolve_event, split_words};
use crate::host::Host;
use k7x::edit::js_number;
use song::clips::ClipKind;
use song_edit::material_cut::{split_audio_placement, split_sequence_material};
use song_edit::track_mix::resize_track_placement;
use song_schema::{beats_to_duration_on, parse_position_on};

/// `event resize <track> <timecode> <newStart> <newEnd>` — both ends, one edit.
pub fn event_resize(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 4 {
        h.refuse([
            "usage: event resize <track> <timecode> <newStart> <newEnd>  (the new END position, not a length)"
                .to_owned(),
        ]);
        return Ok(());
    }
    let (start_tc, end_tc) = (w[2], w[3]);
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        tempo,
    }) = resolve_event(h, w[0], w[1], true, false)?
    else {
        return Ok(());
    };
    let parsed =
        parse_position_on(start_tc, num, den).and_then(|s| parse_position_on(end_tc, num, den).map(|e| (s, e)));
    let (start, end) = match parsed {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    if end <= start {
        h.refuse([format!("the end ({end_tc}) must be after the start ({start_tc})")]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let done = resize_track_placement(h.fs, &dir, &track.id, at_beats, num, den, start, end, tempo)
        .map_err(|e| e.to_string())
        .and_then(|d| {
            beats_to_duration_on(end - start, num, den)
                .map(|s| (d, s))
                .map_err(|e| e.to_string())
        });
    match done {
        Ok((None, _)) => h.refuse([format!("no event on '{}' at {}", track.id, w[1])]),
        Ok((Some(r), span)) => {
            let times = if r.loops > 1.0 {
                format!(" × {}", js_number(r.loops))
            } else {
                String::new()
            };
            h.log([
                format!("{}: {} -> {start_tc}..{end_tc} ({span}){times}", track.id, label(&hit)),
                format!("  this event only — {} keeps its own from/to", r.material),
            ]);
            h.reload();
        }
        Err(m) => h.refuse([format!("event resize failed: {m}")]),
    }
    Ok(())
}

/// `event split <track> <timecode> <at>` — cut the material in two, leaving what was cut on disk.
pub fn event_split(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 3 {
        h.refuse(["usage: event split <track> <timecode> <at>  (where to cut)".to_owned()]);
        return Ok(());
    }
    let (at_tc, cut_tc) = (w[1], w[2]);
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        tempo,
    }) = resolve_event(h, w[0], at_tc, true, false)?
    else {
        return Ok(());
    };
    let cut_beats = match parse_position_on(cut_tc, num, den) {
        Ok(b) => b,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    let dir = h.song_path.clone();
    if hit.kind == ClipKind::Audio {
        if hit.loop_end > hit.loop_start {
            h.refuse([format!(
                "'{}' repeats here — set its loop back to 1 before splitting it",
                hit.name
            )]);
            return Ok(());
        }
        let from = hit.source_from.unwrap_or(0.0);
        let to = hit.source_to.unwrap_or(if hit.length > 0.0 {
            from + hit.length * 60.0 / tempo
        } else {
            0.0
        });
        let cut = from + (cut_beats - at_beats) * 60.0 / tempo;
        if cut_beats <= at_beats + 1e-6 {
            h.refuse(["the cut has to be after the event starts".to_owned()]);
            return Ok(());
        }
        if to > 0.0 && cut >= to - 1e-6 {
            h.refuse(["the cut has to be before the event ends".to_owned()]);
            return Ok(());
        }
        match split_audio_placement(h.fs, &dir, &track.id, at_beats, cut_beats, (num, den), cut) {
            Ok(None) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
            Ok(Some(at)) => {
                h.log([
                    format!(
                        "{}: {} split at {cut_tc}, into two placements of the same file",
                        track.id,
                        label(&hit)
                    ),
                    format!("  the source is cut at {}s — nothing was rendered", js_number(at)),
                ]);
                h.reload();
            }
            Err(e) => h.refuse([format!("event split failed: {e}")]),
        }
        return Ok(());
    }
    match split_sequence_material(h.fs, &dir, &track.id, at_beats, cut_beats, (num, den)) {
        Ok(None) => h.refuse([format!("no event on '{}' at {at_tc}", track.id)]),
        Ok(Some(r)) => {
            h.log([
                format!("{}: {} split at {cut_tc}, into two sequences", track.id, r.from),
                format!(
                    "  {} ({} notes) and {} ({})",
                    r.head.file, r.head.notes, r.tail.file, r.tail.notes
                ),
                format!("  '{}' is untouched — another lane may still be playing it", r.from),
            ]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event split failed: {e}")]),
    }
    Ok(())
}
