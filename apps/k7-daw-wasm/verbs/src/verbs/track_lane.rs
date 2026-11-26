// track_lane.rs — a lane's own verbs: create, order, mute and solo, after verbs/trackLane.ts; automation lanes are M3's.
// A reader would get wrong mute: it is the lane module's bypass in graph.k7x, and solo is project state, `soloed:` in project.k7x.

use super::graph_facts::graph_facts;
use super::shared::{Done, join, split_words, unique_id};
use crate::host::{Host, Thrown};
use k7x::edit::{PathSeg, Value, js_number};
use song::routing::split_port;
use song_edit::graph_edit::{set_bypass_entry, set_module_bypass};
use song_edit::js_text::{js_string, str_to_number};
use song_edit::lane_files::create_track_file;
use song_edit::lane_strip_plan::strip_new_lane;
use song_edit::module_io::entity_file_for;
use song_edit::project_root::load_project_root;
use song_edit::start_time::js_round;
use song_edit::yaml_edit::{delete_file_value, set_file_value};

const TRACK_REF: &str = "@meta/track";

fn key(k: &str) -> Vec<PathSeg> {
    vec![PathSeg::Key(k.to_owned())]
}

/// `track new [name]`, falling back to the next free `track-N`; the lane's strip comes with it.
pub fn track_new(a: &str, h: &mut Host) -> Done {
    let name = split_words(a).first().copied().unwrap_or("");
    let id = if name.is_empty() {
        unique_id("track", &h.project()?)
    } else {
        name.to_owned()
    };
    let dir = h.song_path.clone();
    let written = match create_track_file(h.fs, &dir, &id, TRACK_REF, h.new_id) {
        Ok(w) => w,
        Err(e) => {
            h.refuse([format!("track new: {e}")]);
            return Ok(());
        }
    };
    let (strip, no_strip) = strip_new_lane(h.fs, &h.env, &dir, &id, h.new_id);
    h.reload();
    let mut all: Vec<String> = Vec::new();
    for f in written.into_iter().chain(strip) {
        if !all.contains(&f) {
            all.push(f);
        }
    }
    h.log([format!("+ track {id} ({})", all.join(", "))]);
    if !no_strip.is_empty() {
        h.log([format!("no strip: {no_strip}")]);
    }
    Ok(())
}

/// `orderPosition`: a 1-based slot, or the slot of the lane named.
pub(super) fn order_position(order: &[String], at: &str) -> Option<f64> {
    let n = str_to_number(at);
    if at.trim().is_empty() || !n.is_finite() {
        return order.iter().position(|x| x == at).map(|i| (i + 1) as f64);
    }
    Some(js_round(n))
}

/// `reorderTracks`: `id` moved to the 1-based `pos`, clamped, the whole list stated.
pub(super) fn reorder(order: &[String], id: &str, pos: f64) -> Vec<String> {
    let mut rest: Vec<String> = order.iter().filter(|x| *x != id).cloned().collect();
    let at = (js_round(pos) - 1.0).max(0.0).min(rest.len() as f64) as usize;
    rest.insert(at, id.to_owned());
    rest
}

/// `track order <track> <position|lane>` — states the WHOLE order in project.k7x.
pub fn track_order(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(track_arg), Some(where_arg)) = (w.first().copied(), w.get(1).copied()) else {
        h.refuse([
            "usage: track order <track> <position|lane>  (e.g. track order gtr 2, or track order gtr vo.main)"
                .to_owned(),
        ]);
        return Ok(());
    };
    let p = h.project()?;
    let mut lanes: Vec<String> = Vec::new();
    for t in &p.tracks {
        let lane = split_port(&t.id).0.to_owned();
        if !lanes.contains(&lane) {
            lanes.push(lane);
        }
    }
    let named = |x: &str| -> Option<String> {
        match p.tracks.iter().find(|t| t.id == x || t.name == x) {
            Some(t) => Some(split_port(&t.id).0.to_owned()),
            None => lanes.iter().any(|l| l == x).then(|| x.to_owned()),
        }
    };
    let Some(lane) = named(track_arg) else {
        let hint = if lanes.is_empty() {
            String::new()
        } else {
            format!(" — try: {}", lanes.join(", "))
        };
        h.refuse([format!("no track \"{track_arg}\"{hint}")]);
        return Ok(());
    };
    let target = named(where_arg).unwrap_or_else(|| where_arg.to_owned());
    let Some(pos) = order_position(&lanes, &target) else {
        h.refuse([format!(
            "'{where_arg}' is neither a position nor a lane of this song — try a number 1..{}",
            lanes.len()
        )]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let Some(project) = entity_file_for(&*h.fs, &dir, "", "project") else {
        h.refuse(["track order: no project.k7x — there is nowhere to state a lane order".to_owned()]);
        return Ok(());
    };
    let order = Value::List(reorder(&lanes, &lane, pos).into_iter().map(Value::Str).collect());
    if let Err(e) = set_file_value(h.fs, &project.path, &key("trackOrder"), &order) {
        h.refuse([format!("track order: {e}")]);
        return Ok(());
    }
    let slot = pos.max(1.0).min(lanes.len() as f64);
    h.log([format!(
        "{lane} is now lane {} of {} ({})",
        js_number(slot),
        lanes.len(),
        project.rel.trim_start_matches('/')
    )]);
    h.reload();
    Ok(())
}

/// One lane's bypass: a picture lane's `#audio` half is an entry of the graph's list, held by no node.
fn bypass_track(h: &mut Host, id: &str, bypassed: bool) -> Result<(), Thrown> {
    let (node, port) = split_port(id);
    let picture = port == Some("audio") && graph_facts(h).is_some_and(|f| f.pictures.iter().any(|p| p == node));
    let dir = h.song_path.clone();
    if picture {
        set_bypass_entry(h.fs, &h.env, &dir, id, bypassed)?;
    } else {
        set_module_bypass(h.fs, &h.env, &dir, id, bypassed)?;
    }
    Ok(())
}

/// What a mute or solo addresses: a lane or a group, by id or name.
struct Target {
    id: String,
    mute: bool,
}

fn mix_target(a: &str, h: &mut Host, verb: &str) -> Result<Option<Target>, Thrown> {
    let Some(id) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse([format!("usage: {verb} <track|group>")]);
        return Ok(None);
    };
    let p = h.project()?;
    let lane = p.tracks.iter().find(|x| x.id == id || x.name == id).map(|t| Target {
        id: t.id.clone(),
        mute: t.mute,
    });
    let group = || {
        p.groups.iter().find(|x| x.id == id || x.name == id).map(|g| Target {
            id: g.id.clone(),
            mute: g.mute,
        })
    };
    let Some(t) = lane.or_else(group) else {
        h.refuse([format!("no track \"{id}\"")]);
        return Ok(None);
    };
    Ok(Some(t))
}

/// `track mute <track|group>` — toggles the bypass and pushes the mix.
pub fn track_mute(a: &str, h: &mut Host) -> Done {
    let Some(t) = mix_target(a, h, "mute")? else {
        return Ok(());
    };
    let bypassed = !t.mute;
    h.project()?;
    bypass_track(h, &t.id, bypassed)?;
    h.apply_project();
    h.log([format!("{}: {}", t.id, if bypassed { "muted" } else { "unmuted" })]);
    Ok(())
}

/// `track solo <track|group>` — toggles the lane in `soloed:`, un-muting a muted lane it solos.
pub fn track_solo(a: &str, h: &mut Host) -> Done {
    let Some(t) = mix_target(a, h, "solo")? else {
        return Ok(());
    };
    h.project()?;
    let dir = h.song_path.clone();
    let root = load_project_root(&*h.fs, &dir, false)?;
    let cur: Vec<String> = match root.data.get("soloed") {
        Some(Value::List(l)) => l.iter().map(js_string).collect(),
        _ => Vec::new(),
    };
    let on = !cur.contains(&t.id);
    let next: Vec<String> = if on {
        cur.iter().cloned().chain([t.id.clone()]).collect()
    } else {
        cur.into_iter().filter(|x| *x != t.id).collect()
    };
    let file = join(&dir, "project.k7x");
    if next.is_empty() {
        delete_file_value(h.fs, &file, &key("soloed"))?;
    } else {
        let list = Value::List(next.iter().cloned().map(Value::Str).collect());
        set_file_value(h.fs, &file, &key("soloed"), &list)?;
    }
    if on && t.mute {
        bypass_track(h, &t.id, false)?;
    }
    h.apply_project();
    let soloed = if next.is_empty() {
        String::new()
    } else {
        format!(" — soloed: {}", next.join(", "))
    };
    h.log([format!("{}: solo {}{soloed}", t.id, if on { "on" } else { "off" })]);
    Ok(())
}

/// `track highlight <track>` — the lane joins or leaves `highlighted:`, which the compile plays 10 dB up.
pub fn track_highlight(a: &str, h: &mut Host) -> Done {
    let Some(arg) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: track highlight <track>".to_owned()]);
        return Ok(());
    };
    let p = h.project()?;
    let Some(track) = p.tracks.iter().find(|t| t.id == arg || t.name == arg) else {
        h.refuse([format!("no track \"{arg}\"")]);
        return Ok(());
    };
    let lane = track.id.split('#').next().unwrap_or("").to_owned();
    if let Err(e) = toggle_highlight(h, &lane) {
        h.refuse([format!("track highlight failed: {e}")]);
    }
    Ok(())
}

fn toggle_highlight(h: &mut Host, lane: &str) -> Result<(), song_edit::Error> {
    let dir = h.song_path.clone();
    let root = load_project_root(&*h.fs, &dir, false)?;
    let cur: Vec<String> = match root.data.get("highlighted") {
        Some(Value::List(l)) => l.iter().map(js_string).collect(),
        _ => Vec::new(),
    };
    let on = !cur.iter().any(|x| x == lane);
    let next: Vec<String> = if on {
        cur.into_iter().chain([lane.to_owned()]).collect()
    } else {
        cur.into_iter().filter(|x| x != lane).collect()
    };
    let file = join(&dir, "project.k7x");
    if next.is_empty() {
        delete_file_value(h.fs, &file, &key("highlighted"))?;
    } else {
        set_file_value(
            h.fs,
            &file,
            &key("highlighted"),
            &Value::List(next.into_iter().map(Value::Str).collect()),
        )?;
    }
    h.log([format!(
        "{lane}: highlight {}",
        if on { "on — 10 dB up" } else { "off" }
    )]);
    h.reload();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verbs::shared::free_name;

    #[test]
    fn an_order_is_stated_whole_and_clamped() {
        let o: Vec<String> = ["a", "b", "c"].iter().map(|s| (*s).to_owned()).collect();
        assert_eq!(reorder(&o, "c", 1.0), ["c", "a", "b"]);
        assert_eq!(reorder(&o, "a", 99.0), ["b", "c", "a"]);
        assert_eq!(order_position(&o, "b"), Some(2.0));
        assert_eq!(order_position(&o, "2.6"), Some(3.0));
        assert_eq!(order_position(&o, "z"), None);
    }

    #[test]
    fn a_picture_lane_dropped_on_the_top_row_ends_up_in_front() {
        // Layers are stated bottom first, so the lane in front is the last one.
        let layers: Vec<String> = ["main", "pip", "box"].iter().map(|s| (*s).to_owned()).collect();
        let at = order_position(&layers, "box").unwrap_or(0.0);
        assert_eq!(reorder(&layers, "main", at), ["pip", "box", "main"]);
        assert_eq!(reorder(&layers, "d", 2.0), ["main", "d", "pip", "box"]);
        assert_eq!(reorder(&layers, "b", 2.0), ["main", "b", "pip", "box"]);
    }

    #[test]
    fn a_free_name_is_the_first_unused_number() {
        assert_eq!(free_name("t", &|n| n == "t-1", false, 1), "t-2");
    }
}
