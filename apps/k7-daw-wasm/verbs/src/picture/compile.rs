// compile.rs — a song folder read as PICTURE: the viewport, the lanes in draw order with their runs, lenses and shapes, the markers.
// A reader would get wrong that the graph can stop the picture: an unreadable graph.k7x only takes the runs away, never the shots.

use super::chains::{Chains, Ctx, video_chains};
use super::doc::{Doc, Fade, Marker, Track, Transition};
use super::glsl::read_transition;
use super::lanes::{Media, clip_of};
use super::stretch::{StretchHead, resolve_stretches};
use indexmap::IndexMap;
use k7x::edit::Value;
use song::Cache;
use song::clips::{Grid, Materials, Placer};
use song::files::time_sig_of;
use song::graph::load_graph;
use song::lanes::inputs::{Inputs, load_inputs};
use song::lanes::lanes_of;
use song::lanes::rig::Rig;
use song::lanes::video::video_tracks;
use song::modules::IoKind;
use song::pictures::{LaneShots, ShotIn, Spend, placed_shots_of, with_timeline_shots};
use song_edit::Fs;
use song_edit::confine::Env;
use song_edit::js_text::js_string;
use song_edit::loader::roots_for;
use song_edit::project_root::load_project_root;
use song_edit::start_time::js_to_number;

const NO_VIEWPORT: &str =
    "project.viewport is required in a v1 project — set `viewport: [1280, 720, 30]` (width, height, export fps)";

/// A compiled picture and what it found wrong that still draws.
pub struct Picture {
    pub doc: Doc,
    pub warnings: Vec<String>,
}

fn strings(v: Option<&Value>) -> Option<Vec<String>> {
    match v {
        Some(Value::List(l)) => Some(l.iter().map(js_string).collect()),
        _ => None,
    }
}

/// A wall-clock position in seconds: minutes and seconds only, since `1:30:00` reads as bar:beat:tick.
fn wall_clock(tc: &str) -> Option<f64> {
    let s = tc.trim();
    let num = |x: &str| {
        let (a, b) = x.split_once('.').unwrap_or((x, "0"));
        let d = |y: &str| !y.is_empty() && y.bytes().all(|c| c.is_ascii_digit());
        (d(a) && d(b)).then(|| x.parse::<f64>().ok()).flatten()
    };
    match s.split_once(':') {
        None => num(s),
        Some((m, sec)) => {
            let whole = sec.split('.').next().unwrap_or("");
            if whole.len() != 2 || whole.as_bytes()[0] > b'5' || !m.bytes().all(|c| c.is_ascii_digit()) || m.is_empty()
            {
                return None;
            }
            Some(m.parse::<f64>().ok()? * 60.0 + num(sec)?)
        }
    }
}

/// `markers:` read as wall-clock points, sorted; a key that does not read is skipped.
fn markers_of(v: Option<&Value>) -> Vec<Marker> {
    let Some(Value::Map(pairs)) = v else { return Vec::new() };
    let mut out: Vec<Marker> = pairs
        .iter()
        .filter_map(|(tc, name)| match name {
            Value::Str(n) => wall_clock(tc).map(|sec| Marker {
                sec,
                name: n.clone(),
                tc: tc.clone(),
            }),
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.sec.partial_cmp(&b.sec).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Whether the project draws a picture: lanes of it, or a lens that needs none.
pub fn has_picture(inputs: &Inputs) -> bool {
    !lanes_of(&inputs.local.modules).picture.is_empty()
        || inputs
            .local
            .modules
            .values()
            .any(|m| m.authored.io == Some(IoKind::Camera))
}

/// The shots the placed timelines put on each picture lane, or a warning when the song's grid does not read.
fn placed<'t>(
    inputs: &'t Inputs,
    lanes: &'t song::lanes::Lanes<'t>,
    mats: &'t Materials,
    w: &mut Vec<String>,
) -> LaneShots<'t> {
    let file = inputs.head.root.as_deref().and_then(|r| r.project.as_ref());
    let grid = match file {
        Some(f) => Grid::of_project(f),
        None => time_sig_of(None).map(|s| Grid::new(s, 120.0)),
    };
    match grid {
        Ok(g) => placed_shots_of(lanes, &mut Placer::new(g, mats), &mut Spend::new()),
        Err(e) => {
            w.push(format!("timelines: {e}"));
            LaneShots::new()
        }
    }
}

/// The picture of the project at `dir`, or the sentence that stops it.
///
/// # Errors
/// A project root that does not read, no viewport, or a last open shot that fades out.
pub fn compile(fs: &dyn Fs, cache: &mut Cache, env: &Env, dir: &str) -> Result<Picture, String> {
    let root = load_project_root(fs, dir, true).map_err(|e| e.to_string())?;
    let vp: Vec<f64> = match root.project().and_then(|p| p.get("viewport")) {
        Some(Value::List(l)) => l.iter().map(js_to_number).collect(),
        _ => Vec::new(),
    };
    let at = |i: usize| vp.get(i).copied().filter(|n| *n != 0.0 && !n.is_nan()).unwrap_or(0.0);
    let viewport = [at(0), at(1)];
    let roots = roots_for(fs, env, dir);
    let inputs = load_inputs(fs, cache, &roots, Rig::default()).map_err(|e| e.to_string())?;
    if !has_picture(&inputs) {
        return Ok(Picture {
            doc: Doc::empty(song_edit::paths::basename(dir)),
            warnings: Vec::new(),
        });
    }
    if viewport[0] == 0.0 || viewport[1] == 0.0 {
        return Err(NO_VIEWPORT.to_owned());
    }
    let fps = if at(2) > 0.0 { at(2) } else { 30.0 };
    let mut warnings = Vec::new();
    let lanes = lanes_of(&inputs.local.modules);
    let video = video_tracks(&lanes, &inputs.bypassed);
    let mats = Materials::new(
        &inputs.clips,
        &inputs.sequences,
        &inputs.timelines,
        inputs.audio_paths.clone(),
    );
    let shots = placed(&inputs, &lanes, &mats, &mut warnings);
    let shots = with_timeline_shots(
        video.iter().map(|(n, t)| (*n, t.events)),
        shots,
        &mut Spend::new(),
        &|n| format!("nodes/{n}.k7x"),
    );
    let stated = strings(root.data.get("trackOrder")).unwrap_or_default();
    let names: Vec<&str> = video.keys().copied().collect();
    let mut layers: Vec<&str> = song::doc::order_tracks(&names, &stated)
        .into_iter()
        .map(|i| names[i])
        .collect();
    layers.reverse();
    let errs: Vec<String> = layers
        .iter()
        .filter(|id| {
            video[**id].events.last().is_some_and(|e| e.fade_out.is_some() && e.length.is_none_or(|l| l.0 == 0.0))
        })
        .map(|id| format!("{id}: its last shot fades out but has `length(0)` with nothing after it — give it a length, or put a shot after it"))
        .collect();
    if !errs.is_empty() {
        return Err(format!("video config invalid:\n  - {}", errs.join("\n  - ")));
    }
    let soloed = strings(root.data.get("soloed")).unwrap_or_default();
    let media = Media {
        fs,
        env,
        dir,
        clips: &inputs.clips,
    };
    let tracks: Vec<Track> = layers
        .iter()
        .map(|id| {
            let t = &video[*id];
            let [x, y, w, h] = t.frame.unwrap_or([0.0, 0.0, viewport[0], viewport[1]]);
            let own = shots.get(id).map_or(&[][..], Vec::as_slice);
            Track {
                id: (*id).to_owned(),
                name: (*id).to_owned(),
                color: t.color.unwrap_or("").to_owned(),
                mute: t.mute,
                solo: soloed.iter().any(|s| s == id),
                x,
                y,
                w,
                h,
                clips: own.iter().map(|s| clip_of(&media, s, id, &mut warnings)).collect(),
                ..Track::default()
            }
        })
        .collect();
    let refs = &inputs.vocabulary.modules;
    let cx = Ctx { fs, dir, refs };
    let chains = load_graph(fs, cache, &roots, &inputs).map_or_else(|_| Chains::default(), |g| video_chains(&cx, &g));
    warnings.extend(chains.warnings.iter().cloned());
    let head = StretchHead::of(&inputs);
    let fading = resolve_stretches(tracks, &chains.stretch_lanes, &mut warnings, &head);
    let mut transitions: IndexMap<String, Transition> = IndexMap::new();
    let mut seen = std::collections::HashSet::new();
    let mut stretches = IndexMap::new();
    for c in fading.iter().flat_map(|t| &t.clips) {
        for f in [&c.fade_in, &c.fade_out].into_iter().flatten() {
            if seen.insert(f.with_.clone())
                && let Some(t) = read_transition(fs, dir, &f.with_, refs)
            {
                transitions.insert(f.with_.clone(), t);
            }
        }
        let named = [&c.fade_in, &c.fade_out]
            .into_iter()
            .flatten()
            .filter_map(|f: &Fade| f.chain.clone());
        for name in named.chain(c.effect.clone()) {
            stretches.insert(name.clone(), chains.stretches.get(&name).cloned().unwrap_or_default());
        }
    }
    let drawn = fold(fading, &chains, &mut warnings);
    let timelines = timeline_pictures(&inputs, &media, &drawn, viewport, &mut warnings);
    let mut video_tracks = drawn;
    video_tracks.extend(chains.cameras.iter().map(|cam| {
        let [x, y, w, h] = cam.frame.unwrap_or([0.0, 0.0, viewport[0], viewport[1]]);
        Track {
            id: cam.node.clone(),
            name: cam.node.clone(),
            camera: Some(cam.label.clone()),
            camera_monitor: Some(if cam.monitor == "auto" { "on" } else { cam.monitor }.to_owned()),
            x,
            y,
            w,
            h,
            ..Track::default()
        }
    }));
    let name = root
        .project()
        .and_then(|p| p.get("name"))
        .map_or_else(|| "untitled".to_owned(), js_string);
    let doc = Doc {
        name,
        row_order: strings(root.data.get("trackOrder")),
        viewport,
        fps,
        duration: Doc::duration_of(&video_tracks),
        markers: markers_of(root.data.get("markers")),
        video_tracks,
        frame_plugins: chains.frame.clone(),
        sound: chains.sound,
        transitions,
        stretches,
        timelines: (!timelines.is_empty()).then_some(timelines),
    };
    Ok(Picture { doc, warnings })
}

/// The lanes with their run, their lens and their shapes folded on, all three off the graph.
fn fold(tracks: Vec<Track>, chains: &Chains, warnings: &mut Vec<String>) -> Vec<Track> {
    tracks
        .into_iter()
        .map(|mut t| {
            if let Some(shapes) = chains.shapes_by_lane.get(&t.id) {
                if t.clips.iter().any(|c| !c.id.is_empty()) {
                    warnings.push(format!("{}: shapes are wired to this lane, so its shots draw nothing — an event on a lane of shapes names no clip, it says WHEN the shapes are on", t.id));
                }
                t.shapes = shapes.clone();
            }
            if let Some(p) = chains.by_lane.get(&t.id) {
                t.plugins = p.clone();
            }
            if let Some((label, monitor)) = chains.camera_by_lane.get(&t.id) {
                t.camera = Some(label.clone());
                t.camera_monitor = Some((*monitor).to_owned());
            }
            t
        })
        .collect()
}

/// Each timeline's own picture lanes at its own positions, framed like the song lane of their name.
fn timeline_pictures(
    inputs: &Inputs,
    media: &Media<'_>,
    layers: &[Track],
    viewport: [f64; 2],
    warnings: &mut Vec<String>,
) -> IndexMap<String, Vec<Track>> {
    let mut out = IndexMap::new();
    for (timeline, t) in &inputs.timelines.values {
        let Some(pictures) = t.pictures.as_ref().filter(|p| !p.is_empty()) else {
            continue;
        };
        let lanes = pictures
            .iter()
            .map(|lane| {
                let name = lane.lane.0.as_str();
                let id = format!("{timeline}>{name}");
                let host = layers.iter().find(|l| l.id == name);
                if host.is_none() {
                    warnings.push(format!("{id} draws on the song's picture lane '{name}', and there is none — 'picture lane new {name}' adds it"));
                }
                Track {
                    id: id.clone(),
                    name: name.to_owned(),
                    color: lane.color.clone().unwrap_or_default(),
                    mute: lane.disabled == Some(true),
                    x: host.map_or(0.0, |h| h.x),
                    y: host.map_or(0.0, |h| h.y),
                    w: host.map_or(viewport[0], |h| h.w),
                    h: host.map_or(viewport[1], |h| h.h),
                    clips: lane
                        .events
                        .iter()
                        .flatten()
                        .map(|p| clip_of(media, &ShotIn::own(p), &id, warnings))
                        .collect(),
                    ..Track::default()
                }
            })
            .collect();
        out.insert(timeline.clone(), lanes);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::wall_clock;

    #[test]
    fn a_wall_clock_is_minutes_and_seconds() {
        assert_eq!(wall_clock("90"), Some(90.0));
        assert_eq!(wall_clock("1:30.5"), Some(90.5));
        assert_eq!(wall_clock("1:60"), None);
        assert_eq!(wall_clock("1:30:00"), None);
    }
}
