// emit_audio.rs — a lane's MATERIAL and its placements: the clips and sequences it plays, the audio they read, the lane file.
// What a reader gets wrong: every timing field is a #Timecode STRING, a bare 4 parses as BARS and lands the clip at beat 12.

use k7x::edit::Value;
use sha1::{Digest, Sha1};
use song_edit::material_window::{MaterialWindow, PlacedExtent, material_window};
use song_schema::{beats_to_duration_on, beats_to_position_on};

use crate::emit_ctx::{DocLayout, EmitCtx, number, obj, text, uniquify};
use crate::model::{ClipKind, ClipOut, TrackOut};

struct Placement<'a> {
    id: &'a str,
    start: f64,
    loops: f64,
    fade_in: Option<f64>,
    fade_out: Option<f64>,
    disabled: bool,
}

fn position(ctx: &EmitCtx<'_>, beats: f64) -> Value {
    text(&beats_to_position_on(beats, ctx.meter.0, ctx.meter.1).unwrap_or_default())
}

fn duration(ctx: &EmitCtx<'_>, beats: f64) -> Value {
    text(&beats_to_duration_on(beats, ctx.meter.0, ctx.meter.1).unwrap_or_default())
}

fn placement_of(ctx: &EmitCtx<'_>, p: &Placement<'_>) -> Value {
    let mut out = vec![("ref", text(p.id)), ("start", position(ctx, p.start))];
    // The clip's extent is NOT written here: it belongs to the material.
    if p.loops > 1.0 {
        out.push(("loop", number(p.loops)));
    }
    if p.disabled {
        out.push(("disabled", Value::Bool(true)));
    }
    for (key, v) in [("fadeIn", p.fade_in), ("fadeOut", p.fade_out)] {
        if let Some(v) = v.filter(|v| *v != 0.0) {
            out.push((key, duration(ctx, v)));
        }
    }
    obj(out)
}

/// The window a clip declares, as two quoted POSITION timecodes; a window at the start says so by omission.
fn window_fields(ctx: &EmitCtx<'_>, w: &MaterialWindow) -> Vec<(&'static str, Value)> {
    let mut out = Vec::new();
    if w.from > 0.0 {
        out.push(("from", position(ctx, w.from)));
    }
    if w.to > w.from {
        out.push(("to", position(ctx, w.to)));
    }
    out
}

fn window_of(c: &ClipOut) -> MaterialWindow {
    material_window(&PlacedExtent {
        length: c.length,
        offset: c.offset,
        loop_start: c.loop_start,
        loop_end: c.loop_end,
    })
}

/// The extension of a file name as `path.extname` reads it: from its last dot, none for a leading one.
fn extname(name: &str) -> &str {
    let file = name.rsplit('/').next().unwrap_or(name);
    match file.rfind('.') {
        Some(d) if d > 0 => &file[d..],
        _ => "",
    }
}

/// A name's runs of characters a file name should not hold made one `_`, and cut to `keep` characters.
fn clean(s: &str, keep: usize) -> String {
    let mut out = String::new();
    let mut run = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
            if run {
                out.push('_');
                run = false;
            }
            out.push(c);
        } else {
            run = true;
        }
    }
    if run {
        out.push('_');
    }
    out.chars().take(keep).collect()
}

/// The extension a file lands under: the archive's own, cleaned, since a name from a stranger's zip never reaches a path as it is.
fn safe_ext(source: &str) -> String {
    let ext = clean(extname(source), 16);
    if ext.len() < 2 { ".wav".into() } else { ext }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for b in Sha1::digest(bytes) {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Where these bytes land, named after the file the archive called them and never after a clip.
fn audio_for(ctx: &mut EmitCtx<'_>, source: &str, bytes: &[u8]) -> String {
    let key = hex(bytes);
    if let Some(held) = ctx.audio_rel.get(&key) {
        return held.clone();
    }
    let ext = safe_ext(source);
    let file = source.rsplit('/').next().unwrap_or(source);
    let bare = &file[..file.len() - extname(source).len()];
    let mut stem = clean(bare, 100);
    if stem.is_empty() {
        stem = "audio".into();
    }
    let mut name = format!("{stem}{ext}");
    let mut n = 2;
    while ctx.taken_audio.contains(&name.to_lowercase()) {
        name = format!("{stem}-{n}{ext}");
        n += 1;
    }
    ctx.taken_audio.insert(name.to_lowercase());
    let rel = format!("clips/{name}");
    ctx.audio_rel.insert(key, rel.clone());
    rel
}

/// A MIDI clip: its notes inline in sequences/<id>.k7x, answering its placement.
fn emit_sequence(ctx: &mut EmitCtx<'_>, c: &ClipOut, id: &str) -> Value {
    // Notes go inline as compact {t,d,k,v}, one per line; `v` is omitted at 1.
    let notes: Vec<Value> = c
        .notes
        .iter()
        .map(|n| {
            let mut e = vec![
                ("t", number(n.time)),
                ("d", number(n.duration)),
                ("k", number(n.key)),
            ];
            if n.vel != 1.0 {
                e.push(("v", number(n.vel)));
            }
            obj(e)
        })
        .collect();
    let w = window_of(c);
    let mut fields = window_fields(ctx, &w);
    fields.push(("notes", Value::List(notes)));
    let layout = DocLayout {
        items_one_line: vec![vec!["notes".into()]],
        ..DocLayout::default()
    };
    ctx.write_doc(&format!("sequences/{id}.k7x"), &obj(fields), &layout);
    ctx.sequence_files += 1;
    placement_of(
        ctx,
        &Placement {
            id,
            start: c.start,
            loops: w.loops,
            fade_in: None,
            fade_out: None,
            disabled: c.disabled,
        },
    )
}

/// An audio clip: its file in clips/<id>.k7x, its bytes once per recording, answering its placement.
fn emit_clip(ctx: &mut EmitCtx<'_>, c: &ClipOut, id: &str) -> Value {
    let source = c.source.as_deref();
    let bytes = source.and_then(|s| ctx.assets.get(s)).map(|b| (*b).clone());
    let wav_rel = match (&bytes, source) {
        (Some(b), Some(s)) => audio_for(ctx, s, b),
        _ => format!("clips/{id}{}", safe_ext(source.unwrap_or(""))),
    };
    match bytes {
        Some(b) => {
            let path = format!("{}/{wav_rel}", ctx.project);
            if !ctx.out.binaries.iter().any(|(p, _)| *p == path) {
                ctx.write_binary(path, b);
            }
        }
        None => ctx.missing_audio.push(id.to_owned()),
    }
    // A warped clip carries its stretch settings, so the engine time-stretches it as the source DAW did.
    let w = window_of(c);
    let mut fields = vec![("source", text(&wav_rel))];
    fields.extend(window_fields(ctx, &w));
    if let Some(warp) = &c.warp {
        let markers = warp
            .markers
            .iter()
            .map(|(b, s)| Value::List(vec![number(*b), number(*s)]))
            .collect();
        fields.push((
            "warp",
            obj(vec![
                (
                    "algorithm",
                    text(warp.algorithm.as_deref().unwrap_or("elastique")),
                ),
                ("markers", Value::List(markers)),
            ]),
        ));
    }
    let layout = DocLayout {
        items_one_line: vec![vec!["warp".into(), "markers".into()]],
        ..DocLayout::default()
    };
    ctx.write_doc(&format!("clips/{id}.k7x"), &obj(fields), &layout);
    ctx.clip_files += 1;
    placement_of(
        ctx,
        &Placement {
            id,
            start: c.start,
            loops: w.loops,
            fade_in: c.fade_in,
            fade_out: c.fade_out,
            disabled: c.disabled,
        },
    )
}

/// A lane file: no `name` field, and the mute is the module's bypass rather than anything here.
#[must_use]
pub fn track_value(t: &TrackOut, placements: Vec<Value>) -> Value {
    let mut value = Vec::new();
    if let Some(color) = t.color.as_deref().filter(|c| !c.is_empty()) {
        value.push(("color", text(color)));
    }
    value.push(("extends", text("@meta/track")));
    // The lane file's own key is `events:`, not `clips:`: these are placements of the material.
    value.push(("events", Value::List(placements)));
    obj(value)
}

/// A source track's arrangement and raw material; answers the placement values.
pub fn emit_arrangement(ctx: &mut EmitCtx<'_>, t: &TrackOut, slug: &str) -> Vec<Value> {
    t.clips
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let id = uniquify(&format!("{slug}_{i}"), &mut ctx.used_clip_ids);
            match c.kind {
                ClipKind::Midi => emit_sequence(ctx, c, &id),
                ClipKind::Audio => emit_clip(ctx, c, &id),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_from_a_zip_never_reaches_a_path_with_a_separator() {
        assert_eq!(safe_ext("audio/x.wav"), ".wav");
        assert_eq!(safe_ext("audio/a.\\..\\evil"), "._evil");
        assert_eq!(safe_ext("audio/noext"), ".wav");
        assert_eq!(safe_ext("a/b."), ".wav");
        assert_eq!(clean("bass copy (1)", 100), "bass_copy_1_");
        assert_eq!(clean(&"a".repeat(300), 100).len(), 100);
    }
}
