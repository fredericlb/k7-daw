// picture_doc.rs — the picture as the interchange and the render read it: the lanes in draw order, each shot in decimal seconds.
// A reader would get wrong what is left out: a placed timeline's shots are not read here, only each lane's own `events:`.

use super::shared::join;
use crate::host::Host;
use k7x::edit::Value;
use song::lanes::lanes_of;
use song::pictures::{ShotIn, fade_over, seconds_of};
use song_edit::confine::confine_to_project;
use song_edit::loader::resolve_all;
use song_edit::module_io::entity_file_for;
use song_edit::project_root::load_project_root;
use song_edit::workspace_doc::ws_data;

/// One shot as the picture draws it; `source` is absolute, EMPTY on an activation.
#[derive(Clone, Debug, PartialEq)]
pub struct PicClip {
    pub id: String,
    pub name: String,
    pub source: String,
    pub still: bool,
    pub captions: bool,
    pub start: f64,
    pub length: f64,
    pub source_from: f64,
    pub fade_in: f64,
    pub speed: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PicLane {
    pub id: String,
    pub name: String,
    pub clips: Vec<PicClip>,
}

/// The picture of a project: its name, its frame and rate, and its lanes, back layer first.
#[derive(Clone, Debug, PartialEq)]
pub struct PicDoc {
    pub viewport: [f64; 2],
    pub fps: f64,
    pub lanes: Vec<PicLane>,
}

fn ends_with_any(s: &str, exts: &[&str]) -> bool {
    let lower = s.to_ascii_lowercase();
    exts.iter().any(|e| lower.ends_with(e))
}

/// `loadVideoProject`, as far as a cut needs it.
///
/// # Errors
/// A project with no viewport, or one that does not read.
pub fn load_pictures(h: &Host) -> Result<PicDoc, String> {
    let dir = h.song_path.clone();
    let root = load_project_root(&*h.fs, &dir, true).map_err(|e| e.to_string())?;
    let vp: Vec<f64> = match root.project().and_then(|p| p.get("viewport")) {
        Some(Value::List(l)) => l.iter().map(song_edit::start_time::js_to_number).collect(),
        _ => Vec::new(),
    };
    let at = |i: usize| vp.get(i).copied().filter(|n| *n != 0.0 && !n.is_nan()).unwrap_or(0.0);
    let viewport = [at(0), at(1)];
    if viewport[0] == 0.0 || viewport[1] == 0.0 {
        return Err(
            "project.viewport is required in a v1 project — set `viewport: [1280, 720, 30]` (width, height, export fps)"
                .to_owned(),
        );
    }
    let fps = if at(2) > 0.0 { at(2) } else { 30.0 };
    let resolved = resolve_all(&*h.fs, &h.env, &dir).map_err(|e| e.to_string())?;
    let lanes = lanes_of(&resolved);
    let names: Vec<&str> = lanes
        .sound
        .keys()
        .copied()
        .chain(lanes.picture.keys().copied())
        .collect();
    let stated: Vec<String> = match root.data.get("trackOrder") {
        Some(Value::List(l)) => l.iter().map(song_edit::js_text::js_string).collect(),
        _ => Vec::new(),
    };
    let rows: Vec<&str> = song::doc::order_tracks(&names, &stated)
        .into_iter()
        .map(|i| names[i])
        .collect();
    let pictures: Vec<&str> = rows
        .into_iter()
        .filter(|r| lanes.picture.contains_key(r))
        .rev()
        .collect();
    let out = pictures
        .iter()
        .map(|id| {
            let lane = &lanes.picture[id];
            let clips = lane.events.iter().map(|p| clip_of(h, &dir, &ShotIn::own(p))).collect();
            PicLane {
                id: (*id).to_owned(),
                name: (*id).to_owned(),
                clips,
            }
        })
        .collect();
    Ok(PicDoc {
        viewport,
        fps,
        lanes: out,
    })
}

fn clip_of(h: &Host, dir: &str, s: &ShotIn) -> PicClip {
    let p = s.event;
    let ref_ = p.ref_.as_ref().map_or("", |r| r.0.as_str());
    let start = s.start;
    let length = s.length;
    if ref_.is_empty() {
        return PicClip {
            id: String::new(),
            name: String::new(),
            source: String::new(),
            still: false,
            captions: false,
            start,
            length,
            source_from: 0.0,
            fade_in: 0.0,
            speed: None,
        };
    }
    let raw = entity_file_for(&*h.fs, &join(dir, "clips"), "clips", ref_)
        .and_then(|f| ws_data(&*h.fs, &f.path).ok())
        .and_then(|d| match d.get("source") {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let source = if raw.is_empty() {
        String::new()
    } else {
        confine_to_project(&*h.fs, &h.env, dir, &raw, None).unwrap_or_default()
    };
    let speed = p.speed.filter(|v| v.is_finite() && *v != 1.0);
    PicClip {
        id: ref_.to_owned(),
        name: ref_.to_owned(),
        source,
        still: ends_with_any(&raw, &[".png", ".jpg", ".jpeg", ".webp", ".gif", ".avif"]),
        captions: ends_with_any(&raw, &[".srt", ".vtt"]),
        start,
        length,
        source_from: seconds_of(p.from.as_ref()),
        fade_in: fade_over(p.fade_in.as_ref()),
        speed,
    }
}
