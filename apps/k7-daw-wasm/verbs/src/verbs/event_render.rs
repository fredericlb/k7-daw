// event_render.rs — the verbs whose work is a render or a read of decoded audio: trim, process, consolidate; after eventEdit.ts, clip.ts, eventProcess.ts.
// A reader would get wrong what is ported: every check before the engine is, then the verb refuses by the host member it lacks, writing nothing.
// Consolidating a SEQUENCE renders nothing, so it is ported whole; an audio range wants `render`.

use super::shared::{Done, Found, resolve_event, split_words};
use crate::host::Host;
use song::clips::ClipKind;
use song_edit::js_text::str_to_number;
use song_edit::material_cut::consolidate_sequence_material;
use song_schema::parse_position_on;

/// `event trim <track> <at> [gate-dB]` — checked, then refused: the gate reads the clip's envelope, which only the engine builds.
pub fn event_trim(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if !(2..=3).contains(&w.len()) {
        h.refuse(["usage: event trim <track> <timecode> [gate-dB]  (e.g. event trim vox 24:1 -40)".to_owned()]);
        return Ok(());
    }
    if let Some(g) = w.get(2)
        && (!str_to_number(g).is_finite() || str_to_number(g) > 0.0)
    {
        h.refuse([format!("'{g}' is not a gate in dBFS — a negative number, e.g. -40")]);
        return Ok(());
    }
    if resolve_event(h, w[0], w[1], true, false)?.is_none() {
        return Ok(());
    }
    h.refuse([
        "this host cannot read a clip's envelope — `event trim` needs the engine, which builds it (clipEnvelope)"
            .to_owned(),
    ]);
    Ok(())
}

fn no_render(h: &mut Host, verb: &str) {
    h.refuse([format!(
        "{verb}: no engine to render with — bouncing through a chain means rendering a file (render)"
    )]);
}

/// `track|clip process <target> <chain>` — the target one token, the chain the rest; the bounce is a render.
fn process(a: &str, h: &mut Host, scope: &str) -> Done {
    let w = split_words(a);
    if w.len() < 2 {
        let path = if scope == "clip" { "|path" } else { "" };
        h.refuse([format!(
            "usage: {scope} process <{scope}{path}> <chain>  (a chain of this project, or @k7-core/<path>)"
        )]);
        return Ok(());
    }
    no_render(h, &format!("{scope} process"));
    Ok(())
}

pub fn track_process(a: &str, h: &mut Host) -> Done {
    process(a, h, "track")
}

pub fn clip_process(a: &str, h: &mut Host) -> Done {
    process(a, h, "clip")
}

/// `event process <track> <at> <chain>` — the event is found and checked; its bounce is a render.
pub fn event_process(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse([
            "usage: event process <track> <timecode> <chain>  (e.g. event process gtr 5:1 @k7-core/amp-room)"
                .to_owned(),
        ]);
        return Ok(());
    }
    let (track_arg, at_tc) = (w[0], w[1]);
    let p = h.project()?;
    let Some(track) = p.tracks.iter().find(|t| t.id == track_arg || t.name == track_arg) else {
        h.refuse([format!("no track \"{track_arg}\"")]);
        return Ok(());
    };
    let at = match parse_position_on(
        at_tc,
        f64::from(p.time_sig_numerator),
        f64::from(p.time_sig_denominator),
    ) {
        Ok(b) => b,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    let Some(hit) = track
        .clips
        .iter()
        .find(|c| c.comp.is_none() && (c.start - at).abs() < 1e-6)
    else {
        h.refuse([format!("no event on '{}' at {at_tc}", track.id)]);
        return Ok(());
    };
    if hit
        .event
        .as_ref()
        .and_then(|e| e.comp.as_ref())
        .is_some_and(|c| !c.is_empty())
    {
        h.refuse([format!(
            "the event at {at_tc} is comped from its takes — consolidate it first (selection consolidate)"
        )]);
        return Ok(());
    }
    if hit.kind != ClipKind::Audio {
        h.refuse([format!(
            "the event at {at_tc} is not audio — there is no file to bounce"
        )]);
        return Ok(());
    }
    no_render(h, "event process");
    Ok(())
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
        h.refuse([
            "selection consolidate: no engine to render with — consolidating audio means rendering a file (render)"
                .to_owned(),
        ]);
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
