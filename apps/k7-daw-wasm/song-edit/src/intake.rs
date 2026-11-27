// intake.rs — the names bytes get when they enter a project: a dropped file's clip id, a recorded take's id and line, the clip declaration.
// A reader would get wrong where the accents go: the page folds them with the browser's `normalize` and hands the folded name in (song-edit/docs/buffer.md).

use k7x::edit::{StringifyOptions, Value, js_number, parse, stringify};
use serde::{Deserialize, Serialize};
use song_schema::is_audio;
use song::wire::{CAPTION_EXTS, IMAGE_EXTS, VIDEO_EXTS};

/// Audio or picture: the two kinds of material a lane holds, and the two verbs that place one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum DropKind {
    Audio,
    Video,
}

/// A lane a drop can land on, in the order the timeline draws them.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DropLane {
    pub id: String,
    pub kind: DropKind,
}

/// One recognised file: what it is uploaded as, what it is declared as, and which kind it is.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DroppedClip {
    /// The clip id: the file's stem, since `clips/<id>.k7x` is named by its id and nothing else.
    pub id: String,
    /// The name the bytes land under in `clips/`, extension kept.
    pub upload: String,
    pub kind: DropKind,
}

/// Where one declared clip goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DropPlace {
    pub id: String,
    pub lane: String,
}

/// What a drop will do, decided in one place so the status line reports what actually happened.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DropPlan {
    /// Every recognised file, in drop order: all of them are uploaded and declared, placed or not.
    pub declare: Vec<DroppedClip>,
    /// Where the ones that found a lane go: a subset of `declare`.
    pub place: Vec<DropPlace>,
    /// Names with no kind we can place: a `.txt` on a timeline is a miss, not a clip.
    pub refused: Vec<String>,
}

fn ends_in(name: &str, exts: &[&str]) -> bool {
    let lower = name.to_ascii_lowercase();
    exts.iter().any(|e| lower.strip_suffix(e).is_some_and(|h| h.ends_with('.')))
}

/// Which kind of material a dropped file is: the EXTENSION decides, as it does for the compiler; picture wins the ones both name.
#[must_use]
pub fn dropped_kind(name: &str) -> Option<DropKind> {
    if ends_in(name, VIDEO_EXTS) || ends_in(name, IMAGE_EXTS) || ends_in(name, CAPTION_EXTS) {
        return Some(DropKind::Video);
    }
    is_audio(name).then_some(DropKind::Audio)
}

/// The last segment of a path written with either slash.
fn last_segment(name: &str) -> &str {
    name.rsplit(['/', '\\']).next().unwrap_or("")
}

/// A stem reduced to what a k7x key, a URL and three file systems agree on; the accents are already folded.
fn safe_stem(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let trimmed = out.trim_matches(['-', '.']);
    if trimmed.is_empty() { "clip".to_owned() } else { trimmed.to_owned() }
}

/// A dropped file as it will sit in `clips/`, or None when it is neither kind. `folded` is `name` with its accents taken off.
#[must_use]
pub fn dropped_clip(name: &str, folded: &str) -> Option<DroppedClip> {
    let kind = dropped_kind(name)?;
    // A drag from a folder carries the path it sat at; only the file's own name lands in clips/.
    let base = last_segment(name);
    let dot = base.rfind('.').filter(|&d| d > 0);
    let ext = dot.map_or_else(String::new, |d| base[d..].to_lowercase());
    let folded_base = last_segment(folded);
    let stem = folded_base.strip_suffix(&base[dot.unwrap_or(base.len())..]).unwrap_or(folded_base);
    let id = safe_stem(stem);
    Some(DroppedClip {
        upload: format!("{id}{ext}"),
        id,
        kind,
    })
}

/// The declaration a dropped file gets: `source` and nothing else, the window being the whole file.
///
/// # Errors
/// Never in practice: the one key is printable.
pub fn clip_decl(upload: &str) -> Result<String, k7x::edit::EditError> {
    let value = Value::Map(vec![("source".to_owned(), Value::Str(format!("clips/{upload}")))]);
    stringify(&value, &StringifyOptions { header: &[], inline: None })
}

/// Resolves a drop: `at` names the lane under the pointer, and its kind decides which files can be placed.
#[must_use]
pub fn plan_drop(names: &[(String, String)], lanes: &[DropLane], at: &str) -> DropPlan {
    let mut declare: Vec<DroppedClip> = Vec::new();
    let mut refused = Vec::new();
    for (name, folded) in names {
        let Some(c) = dropped_clip(name, folded) else {
            refused.push(last_segment(name).to_owned());
            continue;
        };
        // Two files of one drop that reduce to the same id (`Kick 1.wav`, `Kick-1.wav`) are two clips, not one.
        let mut id = c.id.clone();
        let mut k = 2;
        while declare.iter().any(|d| d.id == id) {
            id = format!("{}-{k}", c.id);
            k += 1;
        }
        let upload = format!("{id}{}", &c.upload[c.id.len()..]);
        declare.push(DroppedClip { id, upload, kind: c.kind });
    }
    let Some(from) = lanes.iter().position(|l| l.id == at) else {
        return DropPlan {
            declare,
            place: vec![],
            refused,
        };
    };
    let kind = lanes[from].kind;
    // Picture SEQUENCES on the lane it was dropped on, as every video editor does; audio walks down the lanes.
    let targets: Vec<&DropLane> = if kind == DropKind::Video {
        declare.iter().map(|_| &lanes[from]).collect()
    } else {
        lanes[from..].iter().filter(|l| l.kind == kind).collect()
    };
    let place = declare
        .iter()
        .filter(|c| c.kind == kind)
        .enumerate()
        .filter_map(|(i, c)| {
            targets.get(i).map(|l| DropPlace {
                id: c.id.clone(),
                lane: l.id.clone(),
            })
        })
        .collect();
    DropPlan { declare, place, refused }
}

/// How many `-n` suffixes are tried before giving up: a project holding fifty files called `take` has another problem.
pub const MAX_SUFFIX: usize = 50;

/// The `source` a declaration names, or None when it has none or does not read.
#[must_use]
pub fn declared_source(text: &str) -> Option<String> {
    match parse(text).ok()? {
        Value::Map(entries) => match entries.into_iter().find(|(k, _)| k == "source") {
            Some((_, Value::Str(s))) => Some(s),
            _ => None,
        },
        _ => None,
    }
}

/// The candidate ids for `id`, in order: itself, then `id-2`, `id-3`…
#[must_use]
pub fn candidate_ids(id: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| if i == 0 { id.to_owned() } else { format!("{id}-{}", i + 1) }).collect()
}

// ---- a recorded picture take ---------------------------------------------------------------------

/// The extension a container lands under: the compiler reads a clip's KIND off it, so it has to be one it knows.
#[must_use]
pub fn ext_for_mime(mime: Option<&str>) -> &'static str {
    if mime.is_some_and(|m| m.starts_with("video/mp4")) { ".mp4" } else { ".webm" }
}

/// A wall-clock stamp for a filename (`260814T194357`), the shape a recorded audio take already uses; the month counts from 0.
#[must_use]
pub fn file_stamp(year: i32, month0: u32, day: u32, hour: u32, minute: u32, second: u32) -> String {
    format!("{:02}{:02}{day:02}T{hour:02}{minute:02}{second:02}", year.rem_euclid(100), month0 + 1)
}

/// A take's clip id: the lane it was recorded on and when, reduced to what a path and a k7x key hold.
#[must_use]
pub fn take_id(lane: &str, stamp: &str) -> String {
    let mut safe = String::new();
    let mut dash = false;
    for c in lane.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
            safe.push(c);
            dash = false;
        } else if !dash {
            safe.push('-');
            dash = true;
        }
    }
    let safe = safe.trim_start_matches(['-', '.']);
    format!("{}_{stamp}", if safe.is_empty() { "lane" } else { safe })
}

/// The `picture add` line a finished take becomes; the length is MEASURED, since a fresh WebM states none.
#[must_use]
pub fn place_line(lane: &str, id: &str, at_sec: f64, length_sec: f64) -> String {
    // `Math.max(0, NaN)` is NaN, and Rust's `max` would answer 0.
    let s = |n: f64| js_number(crate::start_time::js_round(if n.is_nan() { n } else { n.max(0.0) } * 1000.0) / 1000.0);
    format!("picture add {lane} {id} {} {}", s(at_sec), s(length_sec))
}
