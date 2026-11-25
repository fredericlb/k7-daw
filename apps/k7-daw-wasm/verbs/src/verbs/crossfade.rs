// crossfade.rs — `event crossfade <track> <atA> <atB> <length>`: two events overlapped and faded across each other; after verbs/crossfade.ts.
// A reader would get wrong what moves: B's start is pulled earlier into material it does not play yet, A's end later; 0 drops the fades only.

use super::shared::{Done, meter, split_words};
use crate::host::Host;
use song::PathRule;
use song::clips::audio_def::{ClipGrid, audio_clip_def, material_ref_of};
use song::clips::{ClipDoc, ClipKind};
use song_edit::crossfade::{CrossfadeSide, plan_crossfade};
use song_edit::track_mix::{move_track_placement, set_track_placement_fade, set_track_placement_window};
use song_schema::{TimeSig, beats_to_position_on, parse_duration, parse_position_on};

/// What one side's clip file and wav say: its window, its material's bounds, the file's length.
struct Side {
    side: CrossfadeSide,
    file: f64,
}

fn side_of(h: &Host, c: &ClipDoc, grid: ClipGrid) -> Side {
    let mat = audio_clip_def(&*h.fs, &h.song_path, material_ref_of(&c.id), Some(grid), PathRule::HOST)
        .ok()
        .flatten();
    let file = if c.source.is_empty() {
        0.0
    } else {
        h.seconds(&c.source).unwrap_or(0.0)
    };
    let from = c.source_from.unwrap_or(0.0);
    // The compile leaves an unwindowed clip's length to the engine: its window in seconds is what it lasts.
    let length = if c.length > 0.0 {
        c.length
    } else {
        ((c.source_to.unwrap_or(file) - from) * grid.tempo / 60.0).max(0.0)
    };
    Side {
        side: CrossfadeSide {
            start: c.start,
            length,
            from,
            to: c.source_to,
            mat_from: mat.as_ref().and_then(|m| m.source_from).unwrap_or(0.0),
            mat_to: mat.as_ref().and_then(|m| m.source_to).or((file > 0.0).then_some(file)),
        },
        file,
    }
}

fn has_takes(c: &ClipDoc) -> bool {
    c.comp.is_some() || c.event.as_ref().is_some_and(|e| e.takes.is_some())
}

pub fn event_crossfade(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    if w.len() != 4 {
        h.refuse([
            "usage: event crossfade <track> <atA> <atB> <length>".to_owned(),
            "       (the two events' starts, the length a duration — 0 removes the crossfade)".to_owned(),
        ]);
        return Ok(());
    }
    let (track_arg, at_a, at_b, length_tc) = (w[0], w[1], w[2], w[3]);
    let p = h.project()?;
    let Some(track) = p.tracks.iter().find(|t| t.id == track_arg || t.name == track_arg) else {
        h.refuse([format!("no track \"{track_arg}\"")]);
        return Ok(());
    };
    let (num, den) = meter(&p);
    let parsed = parse_position_on(at_a, num, den).and_then(|x| {
        let y = parse_position_on(at_b, num, den)?;
        let l = parse_duration(length_tc, TimeSig::new(num as u32, den as u32)?)?;
        Ok((x, y, l))
    });
    let (a_beats, b_beats, length) = match parsed {
        Ok(v) => v,
        Err(e) => {
            h.refuse([format!("bad timecode: {e}")]);
            return Ok(());
        }
    };
    let at = |beats: f64| track.clips.iter().position(|c| (c.start - beats).abs() < 1e-6);
    let (Some(ia), Some(ib)) = (at(a_beats), at(b_beats)) else {
        h.refuse([format!("no two events on '{}' at {at_a} and {at_b}", track.id)]);
        return Ok(());
    };
    if ia == ib {
        h.refuse([format!("no two events on '{}' at {at_a} and {at_b}", track.id)]);
        return Ok(());
    }
    let (ca, cb) = (&track.clips[ia], &track.clips[ib]);
    if ca.kind != ClipKind::Audio || cb.kind != ClipKind::Audio {
        h.refuse(["a crossfade is between two AUDIO events — notes do not fade".to_owned()]);
        return Ok(());
    }
    if has_takes(ca) || has_takes(cb) {
        h.refuse([
            "one of these events has takes — crossfade its seams in its takes view (event comp fade)".to_owned(),
        ]);
        return Ok(());
    }
    let dir = h.song_path.clone();
    let id = track.id.clone();
    if length.is_nan() || length <= 0.0 {
        let done = set_track_placement_fade(h.fs, &dir, &id, a_beats, num, den, "fadeOut", 0.0)
            .and_then(|_| set_track_placement_fade(h.fs, &dir, &id, b_beats, num, den, "fadeIn", 0.0));
        match done {
            Ok(_) => {
                h.log([format!("{id}: no crossfade between {at_a} and {at_b}")]);
                h.reload();
            }
            Err(e) => h.refuse([format!("event crossfade failed: {e}")]),
        }
        return Ok(());
    }
    let grid = ClipGrid {
        tempo: p.tempo,
        num,
        den,
    };
    let (sa, sb) = (side_of(h, ca, grid), side_of(h, cb, grid));
    let Some(plan) = plan_crossfade(&sa.side, &sb.side, length, p.tempo) else {
        h.refuse(["neither event has material to overlap with — open one's window first".to_owned()]);
        return Ok(());
    };
    let mut b_at = b_beats;
    let written = (|| -> Result<(), String> {
        let e = |e: song_edit::Error| e.to_string();
        if let Some(a_to) = plan.a_to {
            set_track_placement_window(h.fs, &dir, &id, a_beats, num, den, (sa.side.from, a_to), sa.file).map_err(e)?;
        }
        if (plan.b_start - b_beats).abs() > 1e-9 {
            let to_tc = beats_to_position_on(plan.b_start, num, den).map_err(|x| x.to_string())?;
            move_track_placement(h.fs, &dir, &id, b_beats, &to_tc, num, den).map_err(e)?;
            b_at = plan.b_start;
            let b_to = sb.side.to.unwrap_or(sb.file);
            set_track_placement_window(h.fs, &dir, &id, b_at, num, den, (plan.b_from, b_to), sb.file).map_err(e)?;
        }
        set_track_placement_fade(h.fs, &dir, &id, a_beats, num, den, "fadeOut", plan.length).map_err(e)?;
        set_track_placement_fade(h.fs, &dir, &id, b_at, num, den, "fadeIn", plan.length).map_err(e)?;
        Ok(())
    })();
    match written.and_then(|()| beats_to_position_on(b_at, num, den).map_err(|x| x.to_string())) {
        Ok(b_tc) => {
            h.log([format!(
                "{id}: crossfade of {:.2} beat(s) between {at_a} and {b_tc}",
                plan.length
            )]);
            h.reload();
        }
        Err(e) => h.refuse([format!("event crossfade failed: {e}")]),
    }
    Ok(())
}
