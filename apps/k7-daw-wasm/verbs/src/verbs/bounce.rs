// bounce.rs — a resolved chain run over material: the files a target names, the one-track doc each renders, the clips repointed; after bounce.ts.
// A reader would get wrong where a render lands: beside its source under a stamped name, and only the clip files that played it follow.

use super::chain_resolve::{BOUNCE_CHAIN, ResolvedChain, resolve_chain};
use super::routes::{PEAK, SEPARATE, adopt, job_dir, job_files, render_to};
use crate::host::{Host, Thrown};
use k7x::edit::{PathSeg, Value};
use serde_json::{Value as Json, json};
use song::clips::{ClipDoc, ClipKind};
use song::doc::{PluginDoc, ProjectDoc, TrackDoc};
use song_edit::module_io::entity_files;
use song_edit::paths::{basename, dirname, join, relative, resolve};
use song_edit::workspace_doc::ws_data;
use song_edit::yaml_edit::set_file_value;
use song_schema::round_beat;

/// The param a measuring module receives its number in.
const PEAK_PARAM: &str = "Peak";

/// Where a separation writes, beside its source.
const STEMS_DIR: &str = "stems";

/// Two seconds a millisecond apart are the same end of a file.
const SAME_SEC: f64 = 1e-3;

/// What the material verbs may name: a lane's clips, or one clip or file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Track,
    Clip,
}

impl Scope {
    fn word(self) -> &'static str {
        match self {
            Scope::Track => "track",
            Scope::Clip => "clip",
        }
    }
}

/// The audio files a target names and how a line calls them.
struct Targets {
    label: String,
    sources: Vec<String>,
    track: Option<String>,
}

/// A path the project names, absolute.
fn abs_of(dir: &str, p: &str) -> String {
    if p.starts_with('/') {
        resolve(p)
    } else {
        resolve(&join(&[dir, p]))
    }
}

fn clip_ids(h: &Host) -> Vec<(String, String, String)> {
    let folder = join(&[&h.song_path, "clips"]);
    entity_files(&*h.fs, &folder, "clips")
        .into_iter()
        .map(|(id, f)| (id, f.path, f.rel))
        .collect()
}

/// `resolveTargets`: what to process within what the command promised; each scope refuses the other's argument.
fn resolve_targets(h: &Host, project: &ProjectDoc, target: &str, scope: Scope) -> Result<Targets, String> {
    let rel = song_edit::js_text::js_trim(target).replace('\\', "/");
    let track = project.tracks.iter().find(|t| t.id == rel || t.name == rel);
    let clips = clip_ids(h);
    if scope == Scope::Track {
        let Some(track) = track else {
            let tail = if clips.iter().any(|(id, ..)| *id == rel) {
                " — it is a clip, so run `clip process`".to_owned()
            } else {
                let ids: Vec<&str> = project.tracks.iter().map(|t| t.id.as_str()).collect();
                format!(
                    " (tracks: {})",
                    if ids.is_empty() {
                        "none".to_owned()
                    } else {
                        ids.join(", ")
                    }
                )
            };
            return Err(format!("no track '{rel}'{tail}"));
        };
        let mut sources: Vec<String> = Vec::new();
        for c in &track.clips {
            if c.kind == ClipKind::Audio && !c.source.is_empty() && !sources.contains(&c.source) {
                sources.push(c.source.clone());
            }
        }
        if sources.is_empty() {
            return Err(format!("'{}' has no audio clips to process", track.id));
        }
        return Ok(Targets {
            label: format!("track '{}'", track.id),
            sources,
            track: Some(track.id.clone()),
        });
    }
    if track.is_some() {
        return Err(format!("'{rel}' is a track, so run `track process` instead"));
    }
    if let Some((_, path, file_rel)) = clips.iter().find(|(id, ..)| *id == rel) {
        let raw = ws_data(&*h.fs, path).map_err(|e| e.to_string())?;
        let Some(Value::Str(source)) = raw
            .get("source")
            .filter(|s| !matches!(s, Value::Str(x) if x.is_empty()))
        else {
            return Err(format!("clip '{rel}' names no source ({file_rel})"));
        };
        return Ok(Targets {
            label: format!("clip '{rel}'"),
            sources: vec![abs_of(&h.song_path, source)],
            track: None,
        });
    }
    Ok(Targets {
        label: rel.clone(),
        sources: vec![join(&[&h.song_path, &rel])],
        track: None,
    })
}

/// The name of a file without its folder and its last extension.
fn stem_of(path: &str) -> &str {
    let base = basename(path);
    match base.rfind('.') {
        Some(i) if i > 0 => &base[..i],
        _ => base,
    }
}

/// `name` without a trailing `_<slug>_<YYMMDDTHHMMSS>`, the mark a previous run of the same chain left.
fn without_stamp<'a>(name: &'a str, slug: &str) -> &'a str {
    let suffix = format!("_{slug}_");
    let Some(cut) = name.len().checked_sub(suffix.len() + 13) else {
        return name;
    };
    let Some((head, tail)) = name.split_at_checked(cut) else {
        return name;
    };
    let stamped = tail.strip_prefix(&suffix).is_some_and(|st| {
        st.bytes()
            .enumerate()
            .all(|(i, c)| if i == 6 { c == b'T' } else { c.is_ascii_digit() })
    });
    if stamped { head } else { name }
}

/// `renderPath`: `<name>_<slug>_<stamp>.wav` beside the source; a previous run of the SAME chain is stripped, stamp and all.
pub fn render_path(source: &str, slug: &str, stamp: &str) -> String {
    let base = without_stamp(stem_of(source), slug);
    join(&[&dirname(source), &format!("{base}_{slug}_{stamp}.wav")])
}

/// An audio clip with nothing but its id, name, source and start.
pub fn audio_clip(id: &str, name: &str, source: &str) -> ClipDoc {
    ClipDoc {
        id: id.to_owned(),
        kind: ClipKind::Audio,
        name: name.to_owned(),
        source: source.to_owned(),
        start: 0.0,
        length: 0.0,
        offset: 0.0,
        source_from: None,
        source_to: None,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: Vec::new(),
        warp: None,
        warp_off: None,
        disabled: None,
        event: None,
        comp: None,
        crossfade: None,
        nest: None,
    }
}

/// A document of one track holding `clips` through `plugins`, at the project's tempo and rate.
pub fn one_track_doc(
    project: &ProjectDoc,
    name: &str,
    track: &str,
    clips: Vec<ClipDoc>,
    plugins: Vec<PluginDoc>,
) -> ProjectDoc {
    let mut t = TrackDoc::new(track);
    t.clips = clips;
    t.plugins = plugins;
    let mut doc = ProjectDoc::empty();
    doc.name = name.to_owned();
    doc.tempo = project.tempo;
    doc.sample_rate = project.sample_rate;
    doc.tracks = vec![t];
    doc
}

/// `bounceDoc`: the RAW file at position zero through the chain, no warp and no window.
fn bounce_doc(project: &ProjectDoc, source: &str, chain: &ResolvedChain, plugins: Vec<PluginDoc>) -> ProjectDoc {
    let name = stem_of(source);
    let clip = audio_clip(&format!("{BOUNCE_CHAIN}-clip"), name, source);
    let mut doc = one_track_doc(
        project,
        &format!("{name} · {}", chain.ref_),
        BOUNCE_CHAIN,
        vec![clip],
        plugins,
    );
    doc.tracks[0].name = name.to_owned();
    doc
}

/// `retimeWarpMarkers`: the map's end moved to the file's new length at the map's own rate; None when it already fits.
pub fn retime_warp_markers(markers: &[(f64, f64)], seconds: f64) -> Option<Vec<(f64, f64)>> {
    let (first, last) = (*markers.first()?, *markers.last()?);
    if markers.len() < 2 || seconds.is_nan() || seconds <= 0.0 {
        return None;
    }
    let (src_span, beat_span) = (last.1 - first.1, last.0 - first.0);
    let flat = |x: f64| x.is_nan() || x <= 0.0;
    if flat(src_span) || flat(beat_span) || (last.1 - seconds).abs() <= SAME_SEC {
        return None;
    }
    let rate = beat_span / src_span;
    let mut out: Vec<(f64, f64)> = markers.iter().copied().filter(|m| m.1 < seconds - SAME_SEC).collect();
    // The first point is the anchor and stays whatever the new length is.
    if out.is_empty() {
        out.push(first);
    }
    let at = out[out.len() - 1];
    out.push((round_beat(at.0 + (seconds - at.1) * rate), round_beat(seconds)));
    (out.len() >= 2).then_some(out)
}

fn markers_of(raw: &Value) -> Option<Vec<(f64, f64)>> {
    let Some(Value::List(rows)) = raw.get("warp").and_then(|w| w.get("markers")) else {
        return None;
    };
    let num = |v: Option<&Value>| match v {
        Some(Value::Num(n)) => *n,
        _ => f64::NAN,
    };
    Some(
        rows.iter()
            .map(|r| match r {
                Value::List(p) => (num(p.first()), num(p.get(1))),
                _ => (f64::NAN, f64::NAN),
            })
            .collect(),
    )
}

/// `repointClips`: every clip file that played `from` now plays `to`, its stretch map retimed; `only` limits which.
fn repoint_clips(
    h: &mut Host,
    from: &str,
    to: &str,
    seconds: Option<f64>,
    only: Option<&[String]>,
) -> Result<Vec<String>, Thrown> {
    let dir = h.song_path.clone();
    let rel = relative(&dir, to);
    let mut moved = Vec::new();
    for (id, path, _) in clip_ids(h) {
        if only.is_some_and(|o| !o.contains(&id)) {
            continue;
        }
        // A clip file that won't parse is the compiler's to report, not this edit's.
        let Ok(raw) = ws_data(&*h.fs, &path) else {
            continue;
        };
        let Some(Value::Str(source)) = raw.get("source") else {
            continue;
        };
        if source.is_empty() || abs_of(&dir, source) != resolve(from) {
            continue;
        }
        set_file_value(
            h.fs,
            &path,
            &[PathSeg::Key("source".to_owned())],
            &Value::Str(rel.clone()),
        )?;
        if let (Some(s), Some(markers)) = (seconds, markers_of(&raw))
            && let Some(retimed) = retime_warp_markers(&markers, s)
        {
            let rows = retimed
                .iter()
                .map(|(b, s)| Value::List(vec![Value::Num(*b), Value::Num(*s)]))
                .collect();
            let at = [PathSeg::Key("warp".to_owned()), PathSeg::Key("markers".to_owned())];
            set_file_value(h.fs, &path, &at, &Value::List(rows))?;
        }
        moved.push(id);
    }
    Ok(moved)
}

/// What one run said and whether it wrote anything.
pub struct Processed {
    pub lines: Vec<String>,
    pub changed: bool,
}

fn said(lines: Vec<String>) -> Processed {
    Processed { lines, changed: false }
}

fn first_line(s: &str) -> &str {
    s.split('\n').next().unwrap_or("")
}

/// The cold steps over every source, sequential both ways; a failure stops the rest.
fn run_cold(h: &mut Host, verb: &str, chain: &ResolvedChain, t: &Targets) -> Result<Processed, Thrown> {
    let dir = h.song_path.clone();
    let mut done: Vec<(String, Vec<String>)> = Vec::new();
    for source in &t.sources {
        for step in &chain.cold {
            let args = json!({ "source": source, "model": step.model, "outputs": step.outputs });
            let value = match h.call(SEPARATE, args)? {
                Ok(v) => v,
                Err(f) => return Ok(said(vec![format!("{verb} failed: {}", f.error)])),
            };
            let out_dir = join(&[&dirname(source), STEMS_DIR]);
            let job = job_dir(&value)?;
            let mut files = Vec::new();
            for f in job_files(&value) {
                let dest = format!("{out_dir}/{}", f.out);
                adopt(h, &job, &f.out, &dest)?;
                files.push(dest);
            }
            done.push((source.clone(), files));
        }
    }
    let wrote: usize = done.iter().map(|(_, f)| f.len()).sum();
    let mut lines = vec![format!("{} through '{}' — {wrote} file(s):", t.label, chain.ref_)];
    for (source, files) in &done {
        let shown: Vec<String> = files.iter().map(|f| relative(&dir, f)).collect();
        let list = if shown.is_empty() {
            "nothing".to_owned()
        } else {
            shown.join(", ")
        };
        lines.push(format!("{} → {list}", relative(&dir, source)));
    }
    Ok(Processed {
        lines,
        changed: wrote > 0,
    })
}

/// `processWithChain`: resolve the chain and the material, bounce each file, repoint the clips.
pub fn process_with_chain(
    h: &mut Host,
    project: &ProjectDoc,
    target: &str,
    chain_ref: &str,
    scope: Scope,
    only: Option<&[String]>,
) -> Result<Processed, Thrown> {
    let verb = format!("{} process", scope.word());
    let chain = match resolve_chain(h, project, chain_ref)? {
        Ok(c) => c,
        Err(e) => return Ok(said(vec![format!("{verb}: {e}")])),
    };
    let t = match resolve_targets(h, project, target, scope) {
        Ok(t) => t,
        Err(e) => return Ok(said(vec![format!("{verb}: {e}")])),
    };
    let dir = h.song_path.clone();
    let show = |s: &str| relative(&dir, s);
    let missing: Vec<String> = t.sources.iter().filter(|s| !h.fs.exists(s)).map(|s| show(s)).collect();
    if !missing.is_empty() {
        return Ok(said(vec![format!(
            "missing source(s) for {}: {}",
            t.label,
            missing.join(", ")
        )]));
    }
    if !chain.cold.is_empty() {
        return run_cold(h, &verb, &chain, &t);
    }
    let measuring = chain.plugins.iter().any(|p| p.params.contains_key(PEAK_PARAM));
    let stamp = h.file_stamp();
    let mut lines = Vec::new();
    let mut changed = false;
    for source in &t.sources {
        let out = render_path(source, &chain.slug, &stamp);
        let mut plugins = chain.plugins.clone();
        if measuring {
            // Per source and never written back into the chain, whose next file has its own peak.
            let db = match h.call(PEAK, json!({ "file": source }))? {
                Ok(v) => v.get("db").and_then(Json::as_f64).unwrap_or(0.0),
                Err(f) => {
                    lines.push(format!("{}: {}", show(source), first_line(&f.error)));
                    continue;
                }
            };
            for p in plugins.iter_mut().filter(|p| p.params.contains_key(PEAK_PARAM)) {
                p.params.insert(PEAK_PARAM.to_owned(), db);
            }
        }
        let doc = bounce_doc(project, source, &chain, plugins);
        // Per source, and the loop goes on: one undecodable clip must not cost the others theirs.
        if let Err(e) = render_to(h, &doc, &out)? {
            lines.push(format!("{}: {}", show(source), first_line(&e)));
            continue;
        }
        changed = true;
        let seconds = h.seconds(&out);
        let moved = repoint_clips(h, source, &out, seconds, only)?;
        let tail = match moved.as_slice() {
            [] => " (no clip file named it — nothing to repoint)".to_owned(),
            [one] => format!(" (clip {one} now plays it)"),
            many => format!(" (clips {} now play it)", many.join(", ")),
        };
        lines.push(format!("{} → {}{tail}", show(source), show(&out)));
    }
    if changed {
        lines.insert(
            0,
            format!(
                "{} through '{}' — {} module(s):",
                t.label,
                chain.ref_,
                chain.plugins.len()
            ),
        );
    }
    // Said rather than refused: baking a track's OWN chain in is legitimate once it is bypassed.
    if changed && chain.own_track.is_some() && chain.own_track == t.track {
        lines.push(format!(
            "note: '{}' is the track's own chain and still runs on playback, so the treatment is now heard twice — bypass its modules, or use `track freeze` instead.",
            chain.ref_
        ));
    }
    Ok(Processed { lines, changed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_of_the_same_chain_replaces_its_stamp() {
        assert_eq!(
            render_path("/p/clips/kick.wav", "amp", "261126T120000"),
            "/p/clips/kick_amp_261126T120000.wav"
        );
        assert_eq!(
            render_path("/p/clips/kick_amp_250101T010101.wav", "amp", "261126T120000"),
            "/p/clips/kick_amp_261126T120000.wav"
        );
        assert_eq!(
            render_path("/p/clips/kick_eq_250101T010101.flac", "amp", "261126T120000"),
            "/p/clips/kick_eq_250101T010101_amp_261126T120000.wav"
        );
    }

    #[test]
    fn a_map_follows_a_longer_file_at_its_own_rate() {
        assert_eq!(retime_warp_markers(&[(0.0, 0.0), (4.0, 2.0)], 2.0005), None);
        assert_eq!(
            retime_warp_markers(&[(0.0, 0.0), (4.0, 2.0)], 3.0),
            Some(vec![(0.0, 0.0), (4.0, 2.0), (6.0, 3.0)])
        );
        assert_eq!(
            retime_warp_markers(&[(0.0, 0.0), (2.0, 1.0), (4.0, 2.0)], 1.5),
            Some(vec![(0.0, 0.0), (2.0, 1.0), (3.0, 1.5)])
        );
    }
}
