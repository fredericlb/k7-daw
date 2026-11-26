// picture_shared.rs — what every `picture …` verb opens a lane with: the lane's file, the shot a time names, the edit that refuses.
// A reader would get wrong that a timeline's lane is a map INSIDE the timeline's file, not a file of its own.

use super::picture_file::{Shot, list_shots};
use super::shared::{join, split_words};
use super::words::is_name;
use crate::host::{Host, Thrown};
use k7x::edit::{Document, PathSeg, Scope, Value, js_number};
use song::lanes::lanes_of;
use song_edit::chain_item_edit::{lane_file_for, timeline_lane_of};
use song_edit::error::{Res, fail};
use song_edit::js_text::{js_string, js_trim};
use song_edit::loader::resolve_all;
use song_edit::sugar::strip_doc_ext;
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit, ws_text};
use song_edit::workspace_file::ws_exists;

/// How close two starts have to be to be the same one: the file is written to the millisecond.
pub const SAME: f64 = 5e-4;

/// Where a picture lane's map is: its own file at the root, or a timeline's file at `pictures[i]`.
#[derive(Clone, Debug)]
pub struct LaneAt {
    pub rel: String,
    pub abs: String,
    pub base: Vec<PathSeg>,
    /// The timeline holding it, when it is one of a timeline's.
    pub timeline: Option<String>,
}

/// The project's picture lanes, by node name, in the module map's order.
pub fn picture_lane_ids(h: &Host) -> Result<Vec<String>, Thrown> {
    let resolved = resolve_all(&*h.fs, &h.env, &h.song_path)?;
    Ok(lanes_of(&resolved).picture.keys().map(|k| (*k).to_owned()).collect())
}

/// The placements a lane holds, in file order: the indices every edit writes by.
pub fn shots_of(h: &Host, lane: &LaneAt) -> Result<Vec<Shot>, Thrown> {
    let mut doc = Document::new(&ws_text(&*h.fs, &lane.abs)?);
    Ok(list_shots(&doc.at(&lane.base)))
}

fn names_of(list: Option<&Value>) -> Vec<String> {
    match list {
        Some(Value::List(l)) => l
            .iter()
            .map(|p| p.get("lane").map_or_else(|| "undefined".to_owned(), js_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// The names a timeline's `lanes:` and `pictures:` already hold.
pub fn taken_in(data: &Value) -> Vec<String> {
    let mut out = names_of(data.get("lanes"));
    out.extend(names_of(data.get("pictures")));
    out
}

fn timeline_picture_lane(h: &mut Host, timeline: &str, lane: &str) -> Result<Option<LaneAt>, Thrown> {
    let rel = format!("timelines/{timeline}.k7x");
    let abs = join(&h.song_path, &rel);
    if !is_name(timeline) || !ws_exists(&*h.fs, &abs) {
        h.refuse([format!("no timeline '{timeline}' — {rel} does not exist")]);
        return Ok(None);
    }
    let names = names_of(ws_data(&*h.fs, &abs)?.get("pictures"));
    let Some(at) = names.iter().position(|n| n == lane) else {
        let have = if names.is_empty() {
            String::new()
        } else {
            format!(" — it has {}", names.join(", "))
        };
        h.refuse([format!("timeline '{timeline}' has no picture lane '{lane}'{have}")]);
        return Ok(None);
    };
    Ok(Some(LaneAt {
        rel,
        abs,
        base: vec![PathSeg::Key("pictures".to_owned()), PathSeg::Index(at)],
        timeline: Some(timeline.to_owned()),
    }))
}

/// A lane's file, or a refusal naming the lanes there are; a timeline's lane is its map in the timeline's file.
pub fn lane_file(h: &mut Host, lane: &str) -> Result<Option<LaneAt>, Thrown> {
    let trimmed = js_trim(lane);
    if let Some((timeline, tl_lane)) = timeline_lane_of(trimmed) {
        return timeline_picture_lane(h, timeline, tl_lane);
    }
    let name = strip_doc_ext(trimmed);
    if !is_name(name) {
        h.refuse([format!("'{lane}' is not a lane name")]);
        return Ok(None);
    }
    let ids = picture_lane_ids(h)?;
    let file = ids
        .iter()
        .any(|i| i == name)
        .then(|| lane_file_for(&*h.fs, &h.song_path, name))
        .flatten();
    let Some(file) = file else {
        let have = if ids.is_empty() {
            " — this song has none".to_owned()
        } else {
            format!(" — this song's are {}", ids.join(", "))
        };
        h.refuse([format!("no picture lane '{name}'{have}")]);
        return Ok(None);
    };
    Ok(Some(LaneAt {
        rel: file.rel.trim_start_matches('/').to_owned(),
        abs: file.path,
        base: Vec::new(),
        timeline: None,
    }))
}

/// `lane_file` for a verb about the LAYER (its frame, its visibility), which a timeline's lane borrows from the song's.
pub fn layer_file(h: &mut Host, lane: &str, verb: &str) -> Result<Option<LaneAt>, Thrown> {
    if let Some((_, tl_lane)) = timeline_lane_of(js_trim(lane)) {
        h.refuse([format!(
            "picture {verb}: {lane} draws on the song's picture lane '{tl_lane}' — {verb} that one"
        )]);
        return Ok(None);
    }
    lane_file(h, lane)
}

/// `parseWallClock`: seconds (`12.5`) or `m:ss(.s)`; minutes and seconds only, since `1:30:00` would read as bar:beat:tick.
pub fn parse_wall_clock(tc: &str) -> Option<f64> {
    let s = js_trim(tc);
    let digits = |x: &str| !x.is_empty() && x.bytes().all(|b| b.is_ascii_digit());
    let decimal = |x: &str| match x.split_once('.') {
        Some((a, b)) => digits(a) && digits(b),
        None => digits(x),
    };
    if decimal(s) {
        return s.parse().ok();
    }
    let (m, sec) = s.split_once(':')?;
    let (whole, _) = sec.split_once('.').unwrap_or((sec, ""));
    let tens = whole.as_bytes().first().is_some_and(|b| (b'0'..=b'5').contains(b));
    if digits(m) && whole.len() == 2 && tens && decimal(sec) {
        return Some(m.parse::<f64>().ok()? * 60.0 + sec.parse::<f64>().ok()?);
    }
    None
}

/// A wall-clock argument as seconds, or a refusal in the dialect's own words.
pub fn seconds(h: &mut Host, word: Option<&str>) -> Option<f64> {
    let s = word.and_then(parse_wall_clock);
    if s.is_none() {
        h.refuse([format!(
            "'{}' is not a time — picture reads seconds (12.5) or m:ss (1:30.5)",
            word.unwrap_or("")
        )]);
    }
    s
}

fn placement_at(h: &mut Host, list: &[Shot], at: f64, rel: &str) -> Option<usize> {
    let hits: Vec<usize> = list
        .iter()
        .enumerate()
        .filter(|(_, p)| (p.start - at).abs() < SAME)
        .map(|(i, _)| i)
        .collect();
    match hits.as_slice() {
        [] => {
            let starts: Vec<String> = list.iter().map(|p| js_number(p.start)).collect();
            let have = if starts.is_empty() {
                " — it holds no shot".to_owned()
            } else {
                format!(" — there are shots at {}", starts.join(", "))
            };
            h.refuse([format!("nothing starts at {}s on {rel}{have}", js_number(at))]);
            None
        }
        [i] => Some(*i),
        many => {
            h.refuse([format!(
                "{} shots start at {}s on {rel} — move one of them first, they cannot be told apart",
                many.len(),
                js_number(at)
            )]);
            None
        }
    }
}

/// Edits the lane; an edit answering false writes nothing and is refused with `refusal`.
pub fn put(
    h: &mut Host,
    lane: &LaneAt,
    verb: &str,
    refusal: Option<String>,
    edit: impl FnOnce(&mut Scope) -> Res<bool>,
) -> bool {
    let mut refused = false;
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    let done = ws_edit(h.fs, &lane.abs, opts, |doc| {
        if edit(&mut doc.at(&lane.base))? {
            return Ok(());
        }
        refused = true;
        fail("refused")
    });
    match done {
        Ok(_) => true,
        Err(e) => {
            let said = if refused {
                refusal.unwrap_or_else(|| format!("picture {verb}: nothing to change there"))
            } else {
                format!("picture {verb}: {e}")
            };
            h.refuse([said]);
            false
        }
    }
}

/// What every verb but `add` opens: the lane, its placements, and which one the time names.
pub struct Target {
    pub lane: LaneAt,
    pub list: Vec<Shot>,
    pub at: f64,
    pub index: usize,
    pub rest: Vec<String>,
}

impl Target {
    pub fn shot(&self) -> &Shot {
        &self.list[self.index]
    }
}

pub fn target(h: &mut Host, a: &str, verb: &str, tail: &str) -> Result<Option<Target>, Thrown> {
    let w = split_words(a);
    let (Some(lane_arg), Some(at_arg)) = (w.first(), w.get(1)) else {
        h.refuse([format!("usage: picture {verb} <lane> <at> {tail}")
            .trim_end()
            .to_owned()]);
        return Ok(None);
    };
    let Some(lane) = lane_file(h, lane_arg)? else {
        return Ok(None);
    };
    let Some(at) = seconds(h, Some(at_arg)) else {
        return Ok(None);
    };
    let list = shots_of(h, &lane)?;
    let Some(index) = placement_at(h, &list, at, &lane.rel) else {
        return Ok(None);
    };
    let rest = w[2..].iter().map(|s| (*s).to_owned()).collect();
    Ok(Some(Target {
        lane,
        list,
        at,
        index,
        rest,
    }))
}
