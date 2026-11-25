// picture.rs — the `picture …` verbs that edit a lane and its shots: new, order, mute, solo, rect, move, length, head, delete, split, add.
// A reader would get wrong `head`: it writes THREE fields together, or the tail slides and the frame under the edge changes.

use super::graph_facts::graph_facts;
use super::picture_animate::shift_animation_at;
use super::picture_file::{add_shot, remove_at, set_field_at, set_lane_rect, split_at};
use super::picture_shared::{SAME, lane_file, layer_file, put, seconds, taken_in, target};
use super::shared::{Done, free_name, join, split_words};
use super::track_lane::{order_position, reorder};
use super::words::is_name;
use crate::host::Host;
use k7x::edit::{PathSeg, Value, WriteOptions, js_number};
use song::lanes::lanes_of;
use song_edit::chain_item_edit::{lane_folder, timeline_lane_of};
use song_edit::graph_edit::set_lane_bypass;
use song_edit::js_text::{js_string, js_trim, str_to_number};
use song_edit::lane_files::create_picture_lane;
use song_edit::loader::resolve_all;
use song_edit::module_io::{entity_file_for, entity_files};
use song_edit::project_root::load_project_root;
use song_edit::start_time::js_round;
use song_edit::sugar::strip_doc_ext;
use song_edit::workspace_doc::{EditOpts, ws_data, ws_edit};
use song_edit::workspace_file::ws_exists;
use song_edit::yaml_edit::{delete_file_value, set_file_value};

fn key(k: &str) -> Vec<PathSeg> {
    vec![PathSeg::Key(k.to_owned())]
}

/// `layer-N`, the first free one beside every lane there is: one name space for sound and picture.
fn next_lane_id(h: &Host) -> String {
    let sub = lane_folder(&*h.fs, &h.song_path);
    let taken: Vec<String> = entity_files(&*h.fs, &join(&h.song_path, sub), sub)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    free_name("layer", &|n| taken.iter().any(|t| t == n), false, 1)
}

/// `picture lane new [name]` — `track new`'s counterpart, and the only picture verb that MAKES something.
pub fn lane_new(a: &str, h: &mut Host) -> Done {
    let id = split_words(a)
        .first()
        .map_or_else(|| next_lane_id(h), |s| (*s).to_owned());
    if let Some((timeline, lane)) = timeline_lane_of(&id) {
        return timeline_lane_new(h, timeline, lane);
    }
    let dir = h.song_path.clone();
    match create_picture_lane(h.fs, &h.env, &dir, &id, h.new_id) {
        Ok((written, no_strip)) => {
            let mut lines = vec![format!("+ picture lane {id} ({})", written.join(", "))];
            if !no_strip.is_empty() {
                lines.push(format!("no strip: {no_strip}"));
            }
            h.log(lines);
        }
        Err(e) => {
            h.refuse([format!("picture lane new: {e}")]);
            return Ok(());
        }
    }
    h.reload();
    Ok(())
}

fn timeline_lane_new(h: &mut Host, timeline: &str, lane: &str) -> Done {
    let file = join(&h.song_path, &format!("timelines/{timeline}.k7x"));
    if !is_name(timeline) || !ws_exists(&*h.fs, &file) {
        h.refuse([format!("no timeline '{timeline}'")]);
        return Ok(());
    }
    if !is_name(lane) {
        h.refuse([format!(
            "invalid lane name '{lane}' — letters, digits, '.', '_' and '-' only"
        )]);
        return Ok(());
    }
    if taken_in(&ws_data(&*h.fs, &file)?).iter().any(|t| t == lane) {
        h.refuse([format!("timeline '{timeline}' already has a lane '{lane}'")]);
        return Ok(());
    }
    let entry = Value::Map(vec![
        ("lane".to_owned(), Value::Str(lane.to_owned())),
        ("events".to_owned(), Value::List(vec![])),
    ]);
    let opts = EditOpts {
        create: false,
        raw: false,
    };
    ws_edit(h.fs, &file, opts, |doc| {
        Ok(doc.add_in(&key("pictures"), &entry, WriteOptions::default())?)
    })?;
    h.log([format!("{timeline}: + picture lane {lane}")]);
    h.reload();
    Ok(())
}

/// `picture lane order <lane> <position>` — the draw order is the picture rows' order, so one `trackOrder` write.
pub fn lane_order(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(lane_arg), Some(where_arg)) = (w.first().copied(), w.get(1).copied()) else {
        h.refuse(["usage: picture lane order <lane> <position|lane>  (e.g. picture lane order pip 1)".to_owned()]);
        return Ok(());
    };
    let dir = h.song_path.clone();
    let root = load_project_root(&*h.fs, &dir, true)?;
    let resolved = resolve_all(&*h.fs, &h.env, &dir)?;
    let lanes = lanes_of(&resolved);
    let pictures: Vec<String> = lanes.picture.keys().map(|k| (*k).to_owned()).collect();
    let names: Vec<&str> = lanes
        .sound
        .keys()
        .copied()
        .chain(lanes.picture.keys().copied())
        .collect();
    let stated: Vec<String> = match root.data.get("trackOrder") {
        Some(Value::List(l)) => l.iter().map(js_string).collect(),
        _ => Vec::new(),
    };
    let rows: Vec<String> = song::doc::order_tracks(&names, &stated)
        .into_iter()
        .map(|i| names[i].to_owned())
        .collect();
    let is_pic = |l: &str| pictures.iter().any(|p| p == l);
    let layers: Vec<String> = rows.iter().filter(|l| is_pic(l)).rev().cloned().collect();
    if !layers.iter().any(|l| l == lane_arg) {
        let have = if layers.is_empty() {
            " — this song has none".to_owned()
        } else {
            format!(" — this song's are {}, bottom first", layers.join(", "))
        };
        h.refuse([format!("no picture lane '{lane_arg}'{have}")]);
        return Ok(());
    }
    let Some(pos) = order_position(&layers, where_arg) else {
        h.refuse([format!(
            "'{where_arg}' is neither a position nor a lane of this song — try a number 1..{}",
            layers.len()
        )]);
        return Ok(());
    };
    let mut restacked = reorder(&layers, lane_arg, pos);
    restacked.reverse();
    let mut k = 0;
    let order: Vec<Value> = rows
        .iter()
        .map(|l| {
            if is_pic(l) {
                k += 1;
                Value::Str(restacked[k - 1].clone())
            } else {
                Value::Str(l.clone())
            }
        })
        .collect();
    let Some(project) = entity_file_for(&*h.fs, &dir, "", "project") else {
        h.refuse(["picture lane order: no project.k7x — there is nowhere to state a lane order".to_owned()]);
        return Ok(());
    };
    if let Err(e) = set_file_value(h.fs, &project.path, &key("trackOrder"), &Value::List(order)) {
        h.refuse([format!("picture lane order: {e}")]);
        return Ok(());
    }
    let slot = js_number(pos.max(1.0).min(layers.len() as f64));
    h.log([format!(
        "{lane_arg} is now layer {slot} of {}, bottom first ({})",
        layers.len(),
        project.rel.trim_start_matches('/')
    )]);
    h.reload();
    Ok(())
}

/// `picture mute <lane>` — HIDDEN, there being no sound on a lane: its bypass, as every lane's mute is.
pub fn mute(a: &str, h: &mut Host) -> Done {
    let Some(lane) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: picture mute <lane>".to_owned()]);
        return Ok(());
    };
    if layer_file(h, &lane, "mute")?.is_none() {
        return Ok(());
    }
    let name = strip_doc_ext(js_trim(&lane)).to_owned();
    let muted = graph_facts(h).is_some_and(|f| f.muted_pictures.contains(&name));
    let on = !muted;
    let dir = h.song_path.clone();
    match set_lane_bypass(h.fs, &h.env, &dir, &name, on) {
        Ok(file) => {
            let rel = file.strip_prefix(&format!("{dir}/")).unwrap_or(&file);
            h.log([format!("{name}: {} ({rel})", if on { "hidden" } else { "shown" })]);
        }
        Err(e) => {
            h.refuse([format!("picture mute: {e}")]);
            return Ok(());
        }
    }
    h.reload();
    Ok(())
}

/// `picture solo <lane>` — `soloed:` at project.k7x's root, the SAME list audio uses.
pub fn solo(a: &str, h: &mut Host) -> Done {
    let Some(lane) = split_words(a).first().map(|s| (*s).to_owned()) else {
        h.refuse(["usage: picture solo <lane>".to_owned()]);
        return Ok(());
    };
    if layer_file(h, &lane, "solo")?.is_none() {
        return Ok(());
    }
    let dir = h.song_path.clone();
    let root = load_project_root(&*h.fs, &dir, false)?;
    let cur: Vec<String> = match root.data.get("soloed") {
        Some(Value::List(l)) => l.iter().map(js_string).collect(),
        _ => Vec::new(),
    };
    let on = !cur.contains(&lane);
    let next: Vec<String> = if on {
        cur.iter().cloned().chain([lane.clone()]).collect()
    } else {
        cur.into_iter().filter(|x| *x != lane).collect()
    };
    let file = join(&dir, "project.k7x");
    let written = if next.is_empty() {
        delete_file_value(h.fs, &file, &key("soloed"))
    } else {
        let list = Value::List(next.iter().cloned().map(Value::Str).collect());
        set_file_value(h.fs, &file, &key("soloed"), &list)
    };
    if let Err(e) = written {
        h.refuse([format!("picture solo: {e}")]);
        return Ok(());
    }
    let soloed = if next.is_empty() {
        String::new()
    } else {
        format!(" — soloed: {}", next.join(", "))
    };
    h.log([format!("{lane}: solo {}{soloed}", if on { "on" } else { "off" })]);
    h.reload();
    Ok(())
}

/// `picture rect <lane> <x> <y> <w> <h>` — where in the frame a lane draws; the viewport drag's verb.
pub fn rect(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let nums: Vec<f64> = w.iter().skip(1).map(|n| str_to_number(n)).collect();
    if w.is_empty() || nums.len() != 4 || nums.iter().any(|n| !n.is_finite()) {
        h.refuse(["usage: picture rect <lane> <x> <y> <w> <h>  (e.g. picture rect pip 960 540 320 180)".to_owned()]);
        return Ok(());
    }
    let (x, y, width, height) = (nums[0], nums[1], nums[2], nums[3]);
    if width <= 0.0 || height <= 0.0 {
        h.refuse([format!(
            "a lane {}x{} draws nothing — the width and height ARE the frame it fills, in viewport px",
            js_number(width),
            js_number(height)
        )]);
        return Ok(());
    }
    let Some(found) = layer_file(h, w[0], "rect")? else {
        return Ok(());
    };
    if !put(h, &found, "rect", None, |sc| set_lane_rect(sc, [x, y, width, height])) {
        return Ok(());
    }
    h.log([format!(
        "{}: {}x{} at {},{}",
        found.rel,
        js_number(width),
        js_number(height),
        js_number(x),
        js_number(y)
    )]);
    h.reload();
    Ok(())
}

/// `picture move <lane> <at> <to>` — the body drag: `start`, and nothing else.
pub fn move_shot(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "move", "<to>")? else {
        return Ok(());
    };
    let Some(to) = seconds(h, t.rest.first().map(String::as_str)) else {
        return Ok(());
    };
    let index = t.index;
    if !put(h, &t.lane, "move", None, |sc| set_field_at(sc, index, "start", to)) {
        return Ok(());
    }
    h.log([format!(
        "{}: {} {}s → {}s",
        t.lane.rel,
        t.shot().ref_,
        js_number(t.at),
        js_number(to)
    )]);
    h.reload();
    Ok(())
}

/// `picture length <lane> <at> <seconds>` — the right-edge drag: the extent, 0 meaning "to the end of the source".
pub fn length(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "length", "<seconds>")? else {
        return Ok(());
    };
    let Some(len) = seconds(h, t.rest.first().map(String::as_str)) else {
        return Ok(());
    };
    let index = t.index;
    if !put(h, &t.lane, "length", None, |sc| set_field_at(sc, index, "length", len)) {
        return Ok(());
    }
    let lasts = if len == 0.0 {
        "to the end of its source".to_owned()
    } else {
        format!("{}s", js_number(len))
    };
    h.log([format!("{}: {} lasts {lasts}", t.lane.rel, t.shot().ref_)]);
    h.reload();
    Ok(())
}

/// `picture head <lane> <at> <to>` — the left-edge drag, and the only one that writes THREE fields.
pub fn head(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "head", "<to>")? else {
        return Ok(());
    };
    let Some(to) = seconds(h, t.rest.first().map(String::as_str)) else {
        return Ok(());
    };
    let shot = t.shot().clone();
    let rel = t.lane.rel.clone();
    let delta = to - shot.start;
    if delta.abs() < SAME {
        h.log([format!("{rel}: {} unchanged", shot.ref_)]);
        return Ok(());
    }
    let from = shot.source_from + delta * shot.speed.unwrap_or(1.0);
    if from < -SAME {
        h.refuse([format!(
            "{} starts {}s into its source, so the head cannot move back more than that",
            shot.ref_,
            js_number(shot.source_from)
        )]);
        return Ok(());
    }
    if shot.length > 0.0 && shot.length - delta <= SAME {
        h.refuse([format!(
            "that would leave {} {}ms long — trim the tail instead",
            shot.ref_,
            js_number(js_round((shot.length - delta) * 1000.0))
        )]);
        return Ok(());
    }
    let index = t.index;
    let edit = |sc: &mut k7x::edit::Scope| {
        set_field_at(sc, index, "start", to)?;
        set_field_at(sc, index, "from", from)?;
        if shot.length > 0.0 {
            set_field_at(sc, index, "length", shot.length - delta)?;
        }
        shift_animation_at(sc, index, delta)?;
        Ok(true)
    };
    if !put(h, &t.lane, "head", None, edit) {
        return Ok(());
    }
    let tail = if shot.length > 0.0 {
        ", tail held"
    } else {
        ", length still the source's"
    };
    h.log([format!(
        "{rel}: {} head {}s → {}s{tail}",
        shot.ref_,
        js_number(t.at),
        js_number(to)
    )]);
    h.reload();
    Ok(())
}

/// `picture delete <lane> <at>` — the placement goes; the clip it names does not.
pub fn delete(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "delete", "")? else {
        return Ok(());
    };
    let index = t.index;
    if !put(h, &t.lane, "delete", None, |sc| remove_at(sc, index)) {
        return Ok(());
    }
    h.log([format!(
        "{}: − {} at {}s (the clip stays; a lane holds placements)",
        t.lane.rel,
        t.shot().ref_,
        js_number(t.at)
    )]);
    h.reload();
    Ok(())
}

/// `picture split <lane> <at> <where>` — two placements of one clip, and nothing rendered.
pub fn split(a: &str, h: &mut Host) -> Done {
    let Some(t) = target(h, a, "split", "<where>")? else {
        return Ok(());
    };
    let Some(at_where) = seconds(h, Some(t.rest.first().map_or("", String::as_str))) else {
        return Ok(());
    };
    let index = t.index;
    let outside = format!(
        "{}: {}s is not inside the shot at {}s",
        t.lane.rel,
        js_number(at_where),
        js_number(t.at)
    );
    if !put(h, &t.lane, "split", Some(outside), |sc| split_at(sc, index, at_where)) {
        return Ok(());
    }
    let who = if t.shot().ref_.is_empty() {
        "activation"
    } else {
        &t.shot().ref_
    };
    h.log([format!(
        "{}: {who} at {}s split at {}s",
        t.lane.rel,
        js_number(t.at),
        js_number(at_where)
    )]);
    h.reload();
    Ok(())
}

/// `picture add <lane> <ref> <at> [length] [sourceFrom]` — a shot on a lane, the clip already existing.
pub fn add(a: &str, h: &mut Host) -> Done {
    let w = split_words(a);
    let (Some(lane_arg), Some(ref_), Some(at_arg)) = (w.first().copied(), w.get(1).copied(), w.get(2).copied()) else {
        h.refuse([
            "usage: picture add <lane> <ref> <at> [length] [sourceFrom]  (e.g. picture add main shot_a 12.5 4)"
                .to_owned(),
        ]);
        return Ok(());
    };
    let Some(found) = lane_file(h, lane_arg)? else {
        return Ok(());
    };
    let Some(at) = seconds(h, Some(at_arg)) else {
        return Ok(());
    };
    let length = match w.get(3) {
        None => 0.0,
        Some(l) => match seconds(h, Some(l)) {
            Some(n) => n,
            None => return Ok(()),
        },
    };
    let source_from = match w.get(4) {
        None => 0.0,
        Some(l) => match seconds(h, Some(l)) {
            Some(n) => n,
            None => return Ok(()),
        },
    };
    let unknown = !ws_exists(&*h.fs, &join(&h.song_path, &format!("clips/{ref_}.k7x")));
    let shape = format!(
        "{}: the way `events` is written there is not a shape this can extend — add the line by hand",
        found.rel
    );
    if !put(h, &found, "add", Some(shape), |sc| {
        add_shot(sc, ref_, at, length, source_from)
    }) {
        return Ok(());
    }
    let mut lines = vec![format!(
        "{}: + {ref_} at {}s{}",
        found.rel,
        js_number(at),
        if length != 0.0 {
            format!(" for {}s", js_number(length))
        } else {
            String::new()
        }
    )];
    if unknown {
        lines.push(format!(
            "no clips/{ref_}.k7x — the lane will report it until there is one"
        ));
    }
    h.log(lines);
    h.reload();
    Ok(())
}
