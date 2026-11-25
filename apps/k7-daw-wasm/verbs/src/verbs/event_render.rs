// event_render.rs — the verbs whose work is a render or a read of decoded audio: trim, track and clip process, consolidate; after eventEdit.ts, clip.ts.
// A reader would get wrong where the audio comes from: the envelope and every render are the front's routes, answered on a later run of the line.
// Consolidating a SEQUENCE renders nothing; an audio range is rendered to one clip (material_render.rs).

use super::bounce::{Scope, process_with_chain};
use super::material_render::consolidate_audio;
use super::routes::WAVEFORM;
use super::shared::{Done, Found, label, resolve_event, split_words};
use crate::host::Host;
use k7x::edit::js_number;
use serde_json::{Value as Json, json};
use song::clips::ClipKind;
use song_edit::js_text::str_to_number;
use song_edit::material_cut::consolidate_sequence_material;
use song_edit::track_mix::set_track_placement_window;
use song_schema::parse_position_on;

/// The default gate: below a room's noise floor, above true digital silence.
const DEFAULT_TRIM_DB: f64 = -40.0;

/// What a zero bucket reads as, so comparisons are total.
const MIN_DB: f64 = -100.0;

fn amp_to_db(amp: f64) -> f64 {
    if amp > 0.0 { 20.0 * amp.log10() } else { MIN_DB }
}

/// `trimWindow`: the span of `env` above `gate_db` in seconds, bucket-accurate and clamped to `within`.
fn trim_window(env: &[f64], seconds: f64, gate_db: f64, within: (f64, f64)) -> Option<(f64, f64)> {
    if env.is_empty() || seconds.is_nan() || seconds <= 0.0 {
        return None;
    }
    let per = seconds / env.len() as f64;
    let lo = within.0.min(seconds).max(0.0);
    let hi = within.1.min(seconds).max(lo);
    if hi.is_nan() || hi <= lo {
        return None;
    }
    // The buckets the interval touches: a partly covered one counts.
    let i0 = (lo / per).floor().max(0.0) as usize;
    let i1 = ((hi / per).ceil() - 1.0).min(env.len() as f64 - 1.0);
    if i1 < 0.0 {
        return None;
    }
    let i1 = i1 as usize;
    let above: Vec<usize> = (i0..=i1).filter(|&i| amp_to_db(env[i]) > gate_db).collect();
    let (first, last) = (*above.first()?, *above.last()?);
    let from = lo.max((first as f64 - 1.0) * per);
    let to = hi.min((last as f64 + 2.0) * per);
    // A single bucket over the gate still has to be a window, not a point.
    (to > from).then_some((from, to))
}

/// The clip's whole-source envelope from `/waveform`; any failure is no envelope.
fn envelope(h: &mut Host, clip: &str) -> Result<Option<(Vec<f64>, f64)>, crate::host::Thrown> {
    let Ok(v) = h.call(WAVEFORM, json!({ "clip": clip, "buckets": 2000, "whole": 1 }))? else {
        return Ok(None);
    };
    let rms: Vec<f64> = v
        .get("rms")
        .and_then(Json::as_array)
        .map(|l| l.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect())
        .unwrap_or_default();
    let seconds = v.get("seconds").and_then(Json::as_f64).unwrap_or(0.0);
    Ok((!rms.is_empty()).then_some((rms, seconds)))
}

/// `event trim <track> <at> [gate-dB]` — the silence cut off both ends of one event's window.
pub fn event_trim(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if !(2..=3).contains(&w.len()) {
        h.refuse(["usage: event trim <track> <timecode> [gate-dB]  (e.g. event trim vox 24:1 -40)".to_owned()]);
        return Ok(());
    }
    let gate = w.get(2).map_or(DEFAULT_TRIM_DB, |g| str_to_number(g));
    if !gate.is_finite() || gate > 0.0 {
        h.refuse([format!(
            "'{}' is not a gate in dBFS — a negative number, e.g. -40",
            w.get(2).copied().unwrap_or("")
        )]);
        return Ok(());
    }
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        ..
    }) = resolve_event(h, w[0], w[1], true, false)?
    else {
        return Ok(());
    };
    let name = label(&hit).to_owned();
    let Some((rms, seconds)) = envelope(h, &hit.id)? else {
        h.refuse([format!("{name}: no envelope yet — the engine builds it on first read")]);
        return Ok(());
    };
    // The bound is what this EVENT plays; no `to` is "to the end of the file", as measured.
    let within = (
        hit.source_from.unwrap_or(0.0),
        hit.source_to.filter(|t| *t > 0.0).unwrap_or(seconds),
    );
    let Some(win) = trim_window(&rms, seconds, gate, within) else {
        h.refuse([
            format!(
                "{name}: nothing above {} dB in what this event plays — the gate is too high, or the take is silent",
                js_number(gate)
            ),
            "nothing was changed: a window of zero length would play nothing at all".to_owned(),
        ]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    match set_track_placement_window(h.fs, &dir, &track.id, at_beats, num, den, win, seconds) {
        Ok(true) => {}
        Ok(false) => {
            h.refuse([format!("no event on '{}' at {}", track.id, w[1])]);
            return Ok(());
        }
        Err(e) => {
            h.refuse([format!("event trim failed: {e}")]);
            return Ok(());
        }
    }
    h.log([format!(
        "{}: {name} at {} keeps {:.2}s → {:.2}s of {seconds:.2}s (gate {} dB)",
        track.id,
        w[1],
        win.0,
        win.1,
        js_number(gate)
    )]);
    h.reload();
    Ok(())
}

/// `track|clip process <target> <chain>` — the target one token, the chain the rest of the line.
fn process(a: &str, h: &mut Host, scope: Scope) -> Done {
    let w = split_words(a);
    if w.len() < 2 {
        let (word, path) = match scope {
            Scope::Track => ("track", ""),
            Scope::Clip => ("clip", "|path"),
        };
        h.refuse([format!(
            "usage: {word} process <{word}{path}> <chain>  (a chain of this project, or @k7-core/<path>)"
        )]);
        return Ok(());
    }
    let chain = w[1..].join(" ");
    let p = h.project()?;
    let done = process_with_chain(h, &p, w[0], &chain, scope, None)?;
    h.log(done.lines);
    // Only when something landed: a run that wrote nothing has nothing to re-read.
    if done.changed {
        h.reload();
    }
    Ok(())
}

pub fn track_process(a: &str, h: &mut Host) -> Done {
    process(a, h, Scope::Track)
}

pub fn clip_process(a: &str, h: &mut Host) -> Done {
    process(a, h, Scope::Clip)
}

/// `selection consolidate <track> <from> <to>` — one material over a range of ONE lane, gaps kept.
pub fn selection_consolidate(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 3 {
        h.refuse(["usage: selection consolidate <track> <from> <to>".to_owned()]);
        return Ok(());
    }
    let (from_tc, to_tc) = (w[1], w[2]);
    let mut ids: Vec<&str> = Vec::new();
    for id in w[0].split(',').filter(|s| !s.is_empty()) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    if ids.len() != 1 {
        let given = if ids.is_empty() {
            "none".to_owned()
        } else {
            ids.join(", ")
        };
        h.refuse([format!(
            "selection consolidate takes ONE track — {given} given; a rectangle over several has no single file to write"
        )]);
        return Ok(());
    }
    let Some(Found {
        track,
        at_beats,
        hit,
        num,
        den,
        ..
    }) = resolve_event(h, ids[0], from_tc, false, false)?
    else {
        return Ok(());
    };
    let to_beats = match parse_position_on(to_tc, num, den) {
        Ok(b) => b,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    if hit.kind == ClipKind::Audio {
        let p = h.project()?;
        match consolidate_audio(h, &p, &track.id, (at_beats, to_beats), (num, den))? {
            Ok(None) => h.refuse([format!("nothing on '{}' in that range", track.id)]),
            Ok(Some(done)) => {
                let secs = js_number((done.seconds * 1000.0).round() / 1000.0);
                h.log([
                    format!(
                        "{}: {} event(s) consolidated into {}",
                        track.id, done.replaced, done.file
                    ),
                    format!("  {secs}s rendered from {}", done.from.join(", ")),
                ]);
                h.reload();
            }
            Err(e) => h.refuse([format!("selection consolidate failed: {e}")]),
        }
        return Ok(());
    }
    let dir = h.song_path.clone();
    match consolidate_sequence_material(h.fs, &dir, &track.id, at_beats, to_beats, (num, den)) {
        Ok(None) => h.refuse([format!("nothing on '{}' in that range", track.id)]),
        Ok(Some(done)) => {
            h.log([
                format!(
                    "{}: {} event(s) consolidated into {}",
                    track.id, done.replaced, done.written.file
                ),
                format!("  {} notes, from {}", done.written.notes, done.from.join(", ")),
            ]);
            h.reload();
        }
        Err(e) => h.refuse([format!("selection consolidate failed: {e}")]),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::trim_window;

    #[test]
    fn the_window_is_the_span_above_the_gate_a_bucket_either_side() {
        let env = [0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(trim_window(&env, 10.0, -40.0, (0.0, 10.0)), Some((1.0, 5.0)));
        assert_eq!(trim_window(&env, 10.0, -3.0, (0.0, 10.0)), None);
        assert_eq!(trim_window(&env, 10.0, -40.0, (3.0, 10.0)), Some((3.0, 5.0)));
    }
}
