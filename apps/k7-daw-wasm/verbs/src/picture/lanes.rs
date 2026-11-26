// lanes.rs — the picture lanes as tracks of shots in decimal seconds, each shot's media found, its fades and keys normalised.
// A reader would get wrong what a shot with no ref is: an ACTIVATION, a span saying a lane of shapes is on, never an error.

use super::captions::parse_captions;
use super::doc::{CaptionStyle, Captions, Clip, Fade, Key};
use indexmap::IndexMap;
use song::files::{AnimKey, Animate, Animated, Ease};
use song::pictures::{ShotIn, seconds_of};
use song::{FamilyRead, Span};
use song_edit::Fs;
use song_edit::confine::{Env, confine_to_project};

/// What a shot needs to find its files.
pub struct Media<'a> {
    pub fs: &'a dyn Fs,
    pub env: &'a Env,
    pub dir: &'a str,
    pub clips: &'a FamilyRead<song::Clip>,
}

/// A placement's fade in the one shape every later step copies: a number is a dissolve, a zero is a cut.
pub fn read_fade(f: Option<&song::files::Fade>) -> Option<Fade> {
    let song::files::Fade::Map(m) = f? else { return None };
    let over = m.over.0;
    if over.is_nan() || over <= 0.0 {
        return None;
    }
    let chain = m.chain.as_ref().map(|c| c.0.clone()).filter(|c| !c.is_empty());
    let with_ = m
        .with_
        .as_ref()
        .map(|w| w.0.clone())
        .unwrap_or_else(|| if chain.is_some() { "over" } else { "dissolve" }.to_owned());
    Some(Fade { over, with_, chain })
}

fn ease_word(e: Option<Ease>) -> String {
    let e = e.unwrap_or(Ease::Linear);
    serde_json::to_value(e)
        .ok()
        .and_then(|j| j.as_str().map(str::to_owned))
        .unwrap_or_else(|| "linear".to_owned())
}

/// One property's keys, sorted by time; a constant is one key.
fn keys_of(a: &Animated) -> Vec<Key> {
    let mut keys: Vec<Key> = match a {
        Animated::Const(v) => vec![Key {
            t: 0.0,
            v: *v,
            ease: "linear".to_owned(),
        }],
        Animated::Keys(list) => list
            .iter()
            .map(|k| match k {
                AnimKey::Pair((t, v)) => Key {
                    t: *t,
                    v: *v,
                    ease: "linear".to_owned(),
                },
                AnimKey::Object(o) => Key {
                    t: o.t,
                    v: o.v,
                    ease: ease_word(o.ease),
                },
            })
            .collect(),
    };
    keys.sort_by(|p, q| p.t.partial_cmp(&q.t).unwrap_or(std::cmp::Ordering::Equal));
    keys
}

/// A placement's `animate:` as sorted keys per property, or none.
pub fn read_animate(a: Option<&Animate>) -> Option<IndexMap<String, Vec<Key>>> {
    let a = a?;
    let props = [
        ("x", &a.x),
        ("y", &a.y),
        ("scale", &a.scale),
        ("opacity", &a.opacity),
        ("rotate", &a.rotate),
    ];
    let out: IndexMap<String, Vec<Key>> = props
        .into_iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| (k.to_owned(), keys_of(v))))
        .collect();
    (!out.is_empty()).then_some(out)
}

fn has_ext(s: &str, exts: &[&str]) -> bool {
    song::files::has_ext(s, exts)
}

/// The source, copies and caption style a clip id names, as the clip family holds them.
struct ClipFiles<'c> {
    source: &'c str,
    optimized: Option<&'c str>,
    proxy: Option<&'c str>,
    caption: Option<&'c song::CaptionClip>,
}

fn files_of(c: &song::Clip) -> ClipFiles<'_> {
    match c {
        song::Clip::Audio(a) => ClipFiles {
            source: &a.source.0,
            optimized: None,
            proxy: None,
            caption: None,
        },
        song::Clip::Video(v) => ClipFiles {
            source: &v.source.0,
            optimized: v.optimized.as_deref(),
            proxy: v.proxy.as_deref(),
            caption: None,
        },
        song::Clip::Caption(k) => ClipFiles {
            source: &k.source.0,
            optimized: None,
            proxy: None,
            caption: Some(k),
        },
    }
}

fn word<T: serde::Serialize>(v: Option<&T>) -> Option<String> {
    v.and_then(|v| serde_json::to_value(v).ok())
        .and_then(|j| j.as_str().map(str::to_owned))
}

/// A subtitle clip's cues and style off its file; a file outside the project or unreadable draws nothing, and says so.
fn captions_of(m: &Media<'_>, k: &song::CaptionClip, id: &str, warnings: &mut Vec<String>) -> Captions {
    let source = &k.source.0;
    let Some(path) = confine_to_project(m.fs, m.env, m.dir, source, None) else {
        warnings.push(format!(
            "'{id}': {source} is outside the project — its captions draw nothing"
        ));
        return Captions::default();
    };
    let Some(text) = m.fs.read(&path, Span::All).ok().and_then(|b| String::from_utf8(b).ok()) else {
        warnings.push(format!(
            "'{id}': {source} could not be read — its captions draw nothing"
        ));
        return Captions::default();
    };
    let (cues, problems) = parse_captions(&text);
    warnings.extend(problems.iter().map(|p| format!("'{id}': {source}, {p}")));
    Captions {
        cues,
        style: CaptionStyle {
            font: k.font.clone().filter(|f| !f.is_empty()),
            font_size: k.font_size.as_ref().map(|p| p.0),
            color: k.color.clone().filter(|c| !c.is_empty()),
            background: k.background.clone().filter(|b| !b.is_empty()),
            align: word(k.align.as_ref()),
            position: word(k.position.as_ref()),
            margin: k.margin.as_ref().map(|n| n.0),
        },
    }
}

/// One shot as a clip of the picture, in plain decimal seconds; a ref naming no clip or a file outside is said on `lane`.
pub fn clip_of(m: &Media<'_>, s: &ShotIn<'_>, lane: &str, warnings: &mut Vec<String>) -> Clip {
    let p = s.event;
    let id = p.ref_.as_ref().map_or("", |r| r.0.as_str());
    let animate = read_animate(p.animate.as_ref());
    let timeline = s.timeline.map(str::to_owned);
    if id.is_empty() {
        return Clip {
            start: s.start,
            length: s.length,
            animate,
            timeline,
            ..Clip::default()
        };
    }
    let found = m.clips.get(id).map(|c| files_of(c));
    if found.is_none() {
        warnings.push(format!(
            "{lane}: '{id}' names no clip — add clips/{id}.k7x, or this shot draws nothing"
        ));
    }
    let confine = |r: Option<&str>| {
        r.filter(|r| !r.is_empty())
            .and_then(|r| confine_to_project(m.fs, m.env, m.dir, r, None))
    };
    let raw = found.as_ref().map_or("", |f| f.source);
    let src = if raw.is_empty() {
        String::new()
    } else {
        confine(Some(raw)).unwrap_or_else(|| {
            warnings.push(format!(
                "{lane}: '{id}' names a file outside the project — this shot draws nothing"
            ));
            String::new()
        })
    };
    let speed = p.speed.unwrap_or(1.0);
    Clip {
        id: id.to_owned(),
        name: id.to_owned(),
        src,
        optimized: confine(found.as_ref().and_then(|f| f.optimized)),
        proxy: confine(found.as_ref().and_then(|f| f.proxy)),
        still: has_ext(raw, &["png", "jpg", "jpeg", "webp", "gif", "avif"]),
        start: s.start,
        length: s.length,
        source_from: seconds_of(p.from.as_ref()),
        fade_in: read_fade(p.fade_in.as_ref()),
        fade_out: read_fade(p.fade_out.as_ref()),
        animate,
        speed: (speed.is_finite() && speed != 1.0).then_some(speed),
        effect: p.effect.as_ref().map(|e| e.0.clone()).filter(|e| !e.is_empty()),
        captions: found.and_then(|f| f.caption).map(|k| captions_of(m, k, id, warnings)),
        timeline,
    }
}
