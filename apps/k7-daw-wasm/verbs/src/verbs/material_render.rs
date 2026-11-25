// material_render.rs — the AUDIO half of `selection consolidate`: a range of one lane rendered to one stamped clip; after materialRender.ts.
// A reader would get wrong the gaps: they are rendered as real silence, so the new clip starts where the range does, not at its first event.

use super::bounce::one_track_doc;
use super::routes::render_to;
use crate::host::{Host, Thrown};
use k7x::edit::{StringifyOptions, Value, stringify};
use song::clips::{ClipDoc, ClipKind};
use song::doc::ProjectDoc;
use song_edit::module_io::entity_file_for;
use song_edit::paths::join;
use song_edit::track_mix::{add_track_placements, remove_track_placement};
use song_edit::workspace_doc::ws_write_text;
use song_schema::beats_to_position_on;

/// Two placements at the same beat are the same placement.
const EDGE_EPS: f64 = 1e-6;

/// The one track a cut renders on.
const CUT_TRACK: &str = "cut";

/// What a consolidate wrote: the clip file, the seconds rendered, how many events it replaced and their names.
pub struct Consolidated {
    pub file: String,
    pub seconds: f64,
    pub replaced: usize,
    pub from: Vec<String>,
}

/// A free `clips/<name>`, STAMPED rather than numbered: the wav and its definition share it.
fn free_clip_name(h: &mut Host, base: &str) -> String {
    let stem = format!("{base}_{}", h.file_stamp());
    let folder = join(&[&h.song_path, "clips"]);
    let mut n = 0;
    loop {
        let name = if n == 0 { stem.clone() } else { format!("{stem}-{n}") };
        if entity_file_for(&*h.fs, &folder, "clips", &name).is_none() {
            return name;
        }
        n += 1;
    }
}

/// `consolidateAudioMaterial`: ONE file over `[from, to]` beats, the placements replaced by it; None when the range holds nothing.
///
/// # Errors
/// The inner Err is the sentence `selection consolidate failed:` words; the outer one a pending call.
pub fn consolidate_audio(
    h: &mut Host,
    project: &ProjectDoc,
    track: &str,
    (from, to): (f64, f64),
    (num, den): (f64, f64),
) -> Result<Result<Option<Consolidated>, String>, Thrown> {
    let Some(t) = project.tracks.iter().find(|t| t.id == track) else {
        return Ok(Err(format!("no track '{track}'")));
    };
    let mut picked: Vec<&ClipDoc> = t
        .clips
        .iter()
        .filter(|c| c.start >= from - EDGE_EPS && c.start <= to + EDGE_EPS)
        .collect();
    picked.sort_by(|a, b| a.start.total_cmp(&b.start));
    if picked.is_empty() {
        return Ok(Ok(None));
    }
    if let Some(seq) = picked.iter().find(|c| c.kind != ClipKind::Audio) {
        return Ok(Err(format!(
            "'{}' is a sequence — consolidating notes is text, not a render",
            seq.name
        )));
    }
    let span = (to - from) * 60.0 / project.tempo;
    let clips: Vec<ClipDoc> = picked
        .iter()
        .enumerate()
        .map(|(i, c)| ClipDoc {
            id: format!("{CUT_TRACK}-{i}"),
            // Re-based on the rectangle; everything else about the clip rides along untouched.
            start: c.start - from,
            ..(*c).clone()
        })
        .collect();
    let doc = one_track_doc(project, "consolidate", CUT_TRACK, clips, Vec::new());
    let name = free_clip_name(h, track);
    let dir = h.song_path.clone();
    let wav = join(&[&dir, "clips", &format!("{name}.wav")]);
    if let Err(e) = render_to(h, &doc, &wav)? {
        return Ok(Err(e));
    }
    let file = format!("clips/{name}.k7x");
    let header = ["Rendered by `event consolidate`.".to_owned()];
    let source = Value::Map(vec![("source".to_owned(), Value::Str(format!("clips/{name}.wav")))]);
    let text = stringify(
        &source,
        &StringifyOptions {
            header: &header,
            inline: None,
        },
    )
    .map_err(|e| Thrown(e.to_string()))?;
    ws_write_text(h.fs, &join(&[&dir, &file]), &text)?;
    let seconds = h.seconds(&wav).unwrap_or(span);
    let edit = (|| -> Result<(), String> {
        for c in picked.iter().rev() {
            remove_track_placement(h.fs, &dir, track, c.start, num, den).map_err(|e| e.to_string())?;
        }
        let start = beats_to_position_on(from, num, den).map_err(|e| e.to_string())?;
        let placement = Value::Map(vec![
            ("ref".to_owned(), Value::Str(name.clone())),
            ("start".to_owned(), Value::Str(start)),
        ]);
        add_track_placements(h.fs, &dir, track, &[placement]).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = edit {
        return Ok(Err(e));
    }
    Ok(Ok(Some(Consolidated {
        file,
        seconds,
        replaced: picked.len(),
        from: picked
            .iter()
            .map(|c| {
                if c.name.is_empty() {
                    c.id.clone()
                } else {
                    c.name.clone()
                }
            })
            .collect(),
    })))
}
