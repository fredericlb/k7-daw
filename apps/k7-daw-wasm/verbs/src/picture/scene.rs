// scene.rs — a picture document as the typed scene `@studio/ui-eyes` draws: lanes folded to shown or hidden, passes kept enabled.
// A reader would get wrong that the preview and the export differ only by `use`: a preview reads the proxy, an export never does.

use super::doc::{Clip, Doc, Plugin, Track};
use eyes::schedule::schedule;
use eyes::transitions::with_builtins;
use eyes::{SceneClip, ScenePass, VideoLayer, VideoScene};
use indexmap::IndexMap;

/// Who reads the scene: the live preview, or an export.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Use {
    Preview,
    Export,
}

/// The shader whose knobs at these values change nothing: the compositor skips it there.
const STRIP_SHADER: &str = "video-mixer";

/// Whether a lane's lens is open: `off` never, `on` always, `auto` while the lane is armed.
fn lens_open(t: &Track, armed: Option<&str>) -> bool {
    let Some(_) = &t.camera else { return false };
    match t.camera_monitor.as_deref().unwrap_or("on") {
        "on" => true,
        "auto" => armed == Some(t.id.as_str()),
        _ => false,
    }
}

/// Whether a lane is off screen: muted, soloed away, or fed by a lens that is shut.
fn hidden(all: &[Track], t: &Track, armed: Option<&str>) -> bool {
    t.mute || (all.iter().any(|l| l.solo) && !t.solo) || (t.camera.is_some() && !lens_open(t, armed))
}

/// A shader pass, enabled or not; none for a video effect or a shader with no source.
fn pass_of(p: &Plugin, url_of: &dyn Fn(&str) -> String) -> Option<ScenePass> {
    let s = p.shader.as_ref().filter(|_| p.kind == "shader")?;
    Some(ScenePass {
        id: p.id.clone(),
        name: p.name.clone(),
        vertex: s.vertex.clone(),
        fragment: s.fragment.clone(),
        enabled: p.enabled,
        params: p.params.clone(),
        audio: 0.0,
        lut: p.lut.as_deref().map(url_of),
        identity: (s.name == STRIP_SHADER).then(|| {
            IndexMap::from([("P1", 1.0), ("P2", 1.0), ("P3", 1.0), ("P4", 0.0)].map(|(k, v)| (k.to_owned(), v)))
        }),
    })
}

/// The enabled passes of a run: a bypassed one is dropped so no program is made for it.
fn enabled(run: &[Plugin], url_of: &dyn Fn(&str) -> String) -> Vec<ScenePass> {
    run.iter()
        .filter(|p| p.enabled)
        .filter_map(|p| pass_of(p, url_of))
        .collect()
}

fn clip_of(c: &Clip, use_: Use, url_of: &dyn Fn(&str) -> String) -> SceneClip {
    let file = match use_ {
        Use::Preview => c.proxy.as_deref().or(c.optimized.as_deref()),
        Use::Export => c.optimized.as_deref(),
    };
    let src = if c.src.is_empty() {
        String::new()
    } else {
        url_of(file.unwrap_or(&c.src))
    };
    SceneClip {
        id: c.id.clone(),
        src,
        start: c.start,
        length: c.length,
        source_from: c.source_from,
        fade_in: c.fade_in.clone(),
        fade_out: c.fade_out.clone(),
        animate: c.animate.clone(),
        speed: c.speed,
        still: c.still.then_some(true),
        effect: c.effect.clone(),
        captions: c.captions.clone(),
    }
}

/// The scene of `doc` for `use_`, the lane `armed` opening an `auto` lens, every file made a URL by `url_of`.
pub fn scene_of(doc: &Doc, armed: Option<&str>, use_: Use, url_of: &dyn Fn(&str) -> String) -> VideoScene {
    let all = &doc.video_tracks;
    let mut duration: f64 = 0.0;
    let layers: Vec<VideoLayer> = all
        .iter()
        .map(|t| {
            for c in &t.clips {
                let len = if c.length.is_nan() || c.length == 0.0 {
                    0.0
                } else {
                    c.length
                };
                duration = super::js_max(duration, c.start + len);
            }
            let clips: Vec<SceneClip> = t.clips.iter().map(|c| clip_of(c, use_, url_of)).collect();
            let passes = enabled(&t.plugins, url_of);
            VideoLayer {
                id: t.id.clone(),
                hidden: hidden(all, t, armed),
                camera: lens_open(t, armed).then(|| t.camera.clone()).flatten(),
                x: t.x,
                y: t.y,
                w: t.w,
                h: t.h,
                schedule: schedule(&clips),
                clips,
                shapes: (!t.shapes.is_empty()).then(|| t.shapes.clone()),
                passes: (!passes.is_empty()).then_some(passes),
            }
        })
        .collect();
    let frame = enabled(&doc.frame_plugins, url_of);
    VideoScene {
        viewport: doc.viewport,
        fps: doc.fps,
        layers,
        duration_sec: duration,
        frame_passes: (!frame.is_empty()).then_some(frame),
        transitions: with_builtins(&doc.transitions),
        stretches: (!doc.stretches.is_empty()).then(|| {
            doc.stretches
                .iter()
                .map(|(k, run)| (k.clone(), enabled(run, url_of)))
                .collect()
        }),
    }
}
