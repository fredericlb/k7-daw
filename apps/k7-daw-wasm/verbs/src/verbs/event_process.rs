// event_process.rs — `event process <track> <at> <chain>`: ONE event's material bounced through a chain; after eventProcess.ts.
// A reader would get wrong whose clip follows: an event whose material others play gets its own copy first, so only it changes.

use super::bounce::{Scope, process_with_chain};
use super::shared::{Done, free_name, split_words};
use crate::host::{Host, Thrown};
use k7x::edit::Value;
use song::clips::{ClipKind, material_ref_of};
use song::doc::ProjectDoc;
use song_edit::module_io::entity_files;
use song_edit::paths::{dirname, join};
use song_edit::song_tempo::song_tempo;
use song_edit::track_edit::edit_track_struct;
use song_edit::track_file::{find_placement_at, set_placement_field};
use song_edit::workspace_file::{ws_copy, ws_exists, ws_remove};
use song_schema::parse_position_on;

/// `otherUsesOf`: how many OTHER events play `r`, as their material or as one of their takes.
fn other_uses_of(p: &ProjectDoc, r: &str, track: &str, id: &str) -> usize {
    let mut n = 0;
    for t in &p.tracks {
        for c in &t.clips {
            // A comped event's pieces are one event: only its own clip counts.
            if c.comp.is_some() || (t.id == track && c.id == id) {
                continue;
            }
            let takes = c.event.as_ref().and_then(|e| e.takes.as_ref());
            if material_ref_of(&c.id) == r || takes.is_some_and(|k| k.iter().any(|k| k.ref_.0 == r)) {
                n += 1;
            }
        }
    }
    n
}

/// Points the placement at `at` on `track` at `r`; whether one was there.
fn set_ref(h: &mut Host, track: &str, at: (f64, f64, f64), r: &str, what: &str) -> Result<bool, String> {
    let dir = h.song_path.clone();
    let tempo = song_tempo(&*h.fs, &dir).map_err(|e| e.to_string())?;
    let mut hit = false;
    edit_track_struct(h.fs, &dir, track, what, |doc| {
        if let Some(i) = find_placement_at(doc, at.0, at.1, at.2, tempo)? {
            hit = true;
            set_placement_field(doc, i, "ref", Some(Value::Str(r.to_owned())))?;
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;
    Ok(hit)
}

/// `ownMaterial`: the event at `at` given its own copy of clip `r`; the copy's id.
fn own_material(h: &mut Host, track: &str, at: (f64, f64, f64), r: &str) -> Result<String, String> {
    let clips = entity_files(&*h.fs, &join(&[&h.song_path, "clips"]), "clips");
    let Some((_, src)) = clips.iter().find(|(id, _)| id == r) else {
        return Err(format!("clip '{r}' has no file in clips/"));
    };
    let id = free_name(r, &|n| clips.iter().any(|(c, _)| c == n), false, 2);
    let to = join(&[&dirname(&src.path), &format!("{id}.k7x")]);
    ws_copy(h.fs, &src.path, &to).map_err(|e| e.to_string())?;
    if !set_ref(h, track, at, &id, "give an event its own clip")? {
        ws_remove(h.fs, &to).map_err(|e| e.to_string())?;
        return Err(format!("no event on '{track}' at that position"));
    }
    Ok(id)
}

/// `event process <track> <at> <chain>` — the chain is the rest of the line, like `clip process`.
pub fn event_process(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() < 3 {
        h.refuse([
            "usage: event process <track> <timecode> <chain>  (e.g. event process gtr 5:1 @k7-core/amp-room)"
                .to_owned(),
        ]);
        return Ok(());
    }
    let (track_arg, at_tc, chain) = (w[0], w[1], w[2..].join(" "));
    let p = h.project()?;
    let Some(track) = p.tracks.iter().find(|t| t.id == track_arg || t.name == track_arg) else {
        h.refuse([format!("no track \"{track_arg}\"")]);
        return Ok(());
    };
    let (num, den) = (f64::from(p.time_sig_numerator), f64::from(p.time_sig_denominator));
    let at = match parse_position_on(at_tc, num, den) {
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
    let track_id = track.id.clone();
    let r = material_ref_of(&hit.id).to_owned();
    let shared = other_uses_of(&p, &r, &track_id, &hit.id) > 0;
    let place = (at, num, den);
    let own = if shared {
        match own_material(h, &track_id, place, &r) {
            Ok(id) => id,
            Err(e) => {
                h.refuse([format!("event process failed: {e}")]);
                return Ok(());
            }
        }
    } else {
        r.clone()
    };
    let done = process_with_chain(h, &p, &own, &chain, Scope::Clip, Some(std::slice::from_ref(&own)))?;
    // A bounce that wrote nothing leaves the event as it was found, not on a copy it never needed.
    if shared && !done.changed {
        set_ref(h, &track_id, place, &r, "restore an event's clip").map_err(Thrown)?;
        let copy = join(&[&h.song_path, "clips", &format!("{own}.k7x")]);
        if ws_exists(&*h.fs, &copy) {
            ws_remove(h.fs, &copy)?;
        }
    }
    let mut lines = done.lines;
    if shared && done.changed {
        lines.insert(
            0,
            format!("{track_id} at {at_tc}: its own clip '{own}', the others keep '{r}'"),
        );
    }
    h.log(lines);
    if done.changed {
        h.reload();
    }
    Ok(())
}
