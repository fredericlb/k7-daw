// project.rs — the project-wide verbs of M1: tempo, meter, markers and reload, after verbs/project.ts.
// A reader would get wrong a marker's key: its CANONICAL timecode, so `5` and `5:1:1` name one marker.

use super::shared::{Done, join, meter, position};
use crate::host::Host;
use k7x::edit::{PathSeg, Value, js_number};
use song_edit::js_text::{is_js_space, js_trim, str_to_number};
use song_edit::start_time::js_round;
use song_edit::workspace_doc::{EditOpts, ws_edit};
use song_edit::yaml_edit::set_file_value;
use song_schema::beats_to_position_on;

const MIN_BPM: f64 = 20.0;
const MAX_BPM: f64 = 400.0;

fn path(keys: &[&str]) -> Vec<PathSeg> {
    keys.iter().map(|k| PathSeg::Key((*k).to_owned())).collect()
}

/// `project tempo <bpm>` — the song's one tempo, to two decimals.
pub fn project_tempo(a: &str, h: &mut Host) -> Done {
    let bpm = str_to_number(a);
    if !(MIN_BPM..=MAX_BPM).contains(&bpm) {
        h.refuse([format!(
            "usage: project tempo <bpm> — a number from {MIN_BPM} to {MAX_BPM} (e.g. project tempo 96)"
        )]);
        return Ok(());
    }
    let tidy = js_round(bpm * 100.0) / 100.0;
    let file = join(&h.song_path, "project.k7x");
    set_file_value(h.fs, &file, &path(&["project", "tempo"]), &Value::Num(tidy))?;
    h.log([format!("tempo {} bpm", js_number(tidy))]);
    h.reload();
    Ok(())
}

/// `project meter <beats>/<unit>` — the unit a power of two, at most 32 beats a bar.
pub fn project_meter(a: &str, h: &mut Host) -> Done {
    let parsed = js_trim(a).split_once('/').and_then(|(b, u)| {
        let digits = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
        (digits(b) && digits(u)).then(|| (str_to_number(b), str_to_number(u)))
    });
    let ok = parsed.is_some_and(|(b, u)| (1.0..=32.0).contains(&b) && [1.0, 2.0, 4.0, 8.0, 16.0, 32.0].contains(&u));
    let Some((beats, unit)) = parsed.filter(|_| ok) else {
        h.refuse(["usage: project meter <beats>/<unit> (e.g. project meter 3/4, 6/8)".to_owned()]);
        return Ok(());
    };
    let sig = format!("{}/{}", js_number(beats), js_number(unit));
    let file = join(&h.song_path, "project.k7x");
    set_file_value(
        h.fs,
        &file,
        &path(&["project", "timeSignature"]),
        &Value::Str(sig.clone()),
    )?;
    h.log([format!("time signature {sig}")]);
    h.reload();
    Ok(())
}

/// Makes the project's `markers` map equal `map`, touching only the markers that changed.
fn write_markers(h: &mut Host, map: Vec<(String, Value)>) -> Done {
    let file = join(&h.song_path, "project.k7x");
    let opts = EditOpts {
        create: true,
        raw: false,
    };
    ws_edit(h.fs, &file, opts, |doc| {
        Ok(doc.apply_in(&path(&["markers"]), &Value::Map(map))?)
    })?;
    Ok(())
}

/// `project marker new <timecode> <name>` — adds or renames the marker at that spot.
pub fn marker_create(a: &str, h: &mut Host) -> Done {
    let trimmed = js_trim(a);
    let (tc_raw, name) = match trimmed.find(is_js_space) {
        Some(sp) => (&trimmed[..sp], js_trim(&trimmed[sp..])),
        None => ("", ""),
    };
    if tc_raw.is_empty() || name.is_empty() {
        h.refuse(["usage: create marker <timecode> <name>  (e.g. create marker 5:1:1 Chorus)".to_owned()]);
        return Ok(());
    }
    let p = h.project()?;
    let (num, den) = meter(&p);
    let beat = match position(tc_raw, &p) {
        Ok(b) => b,
        Err(m) => {
            h.refuse([format!("bad timecode: {m}")]);
            return Ok(());
        }
    };
    let tc = beats_to_position_on(beat, num, den).map_err(|e| crate::host::Thrown(e.to_string()))?;
    let mut all: Vec<(String, String, f64)> = p
        .markers
        .iter()
        .filter(|m| m.tc != tc)
        .map(|m| (m.tc.clone(), m.name.clone(), m.beat))
        .collect();
    all.push((tc.clone(), name.to_owned(), beat));
    all.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
    write_markers(h, all.into_iter().map(|(t, n, _)| (t, Value::Str(n))).collect())?;
    h.log([format!("marker \"{name}\" @ {tc}")]);
    h.reload();
    Ok(())
}

/// `project marker delete <name>` — every marker of that name.
pub fn marker_delete(a: &str, h: &mut Host) -> Done {
    let name = js_trim(a);
    if name.is_empty() {
        h.refuse(["usage: project marker delete <name>".to_owned()]);
        return Ok(());
    }
    let p = h.project()?;
    let kept: Vec<(String, Value)> = p
        .markers
        .iter()
        .filter(|m| m.name != name)
        .map(|m| (m.tc.clone(), Value::Str(m.name.clone())))
        .collect();
    if kept.len() == p.markers.len() {
        h.refuse([format!("no marker named \"{name}\"")]);
        return Ok(());
    }
    write_markers(h, kept)?;
    h.log([format!("deleted marker \"{name}\"")]);
    h.reload();
    Ok(())
}

/// `project reload` — the files read again.
pub fn project_reload(_a: &str, h: &mut Host) -> Done {
    h.reload();
    Ok(())
}
