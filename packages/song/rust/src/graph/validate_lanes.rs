// graph/validate_lanes.rs — the passes of validate over lanes and modules: clips, assets, live inputs, channels, groups, assemblies.
// A reader would get wrong that a parked chain is skipped by the input pass and an input with no lane is no fault (docs/liveProject.md).

use super::issues::{WarningCode as C, cut, issue};
use super::model::Channel;
use super::validate::Walk;
use super::{ClipKind, GraphDoc, MAX_ISSUES, TrackType};
use crate::lanes::live::input_landing_track;
use crate::lanes::rig::Rig;
use crate::modules::{IoKind, ModuleKind};
use crate::routing::is_parked;
use std::collections::HashMap;

fn is_input(m: &super::GraphModule<'_>) -> bool {
    m.kind == ModuleKind::Io && m.io == Some(IoKind::In)
}

/// Whether the resolved file a file-valued param names is there (416: `ir` is read off the resolved module).
fn resolved_file(m: &super::GraphModule<'_>, param: &str) -> bool {
    let file = match param {
        "model" => m.m.model.as_deref(),
        "onnx" => m.m.onnx.as_deref(),
        "ir" => m.m.ir.as_deref(),
        _ => None,
    };
    file.is_some_and(|f| !f.is_empty())
}

pub(super) fn lanes(w: &mut Walk<'_, '_>) {
    clips(w);
    assets(w);
    inputs(w);
    groups(w);
    assemblies(w);
}

fn clips(w: &mut Walk<'_, '_>) {
    for (id, t) in &w.g.tracks {
        for p in &t.placements {
            if p.kind == ClipKind::Missing {
                w.sink
                    .add(|| issue(C::TrackClipMissing, [("track", cut(id)), ("ref", cut(p.ref_))]));
            }
        }
    }
}

/// A module naming a file its own assets/ does not hold: `nam` and `assets` disagreeing IS the report.
fn assets(w: &mut Walk<'_, '_>) {
    for (name, m) in &w.g.modules {
        for (param, r) in m.m.assets.iter().flatten() {
            if !resolved_file(m, param) {
                w.sink.add(|| {
                    issue(
                        C::ModuleAssetMissing,
                        [("module", cut(name)), ("param", cut(param)), ("ref", cut(r))],
                    )
                });
            }
        }
    }
}

/// Live-input modules, walked over the CHAINS because an input is wired by the run it sits in.
fn inputs(w: &mut Walk<'_, '_>) {
    let g = w.g;
    let mut fed_by: HashMap<&str, &str> = HashMap::new();
    for c in g.chains.values() {
        if is_parked(c) {
            continue;
        }
        // The landing depends on the chain alone: it is found once, at the first input (441).
        let mut landing: Option<Option<&str>> = None;
        for s in &c.use_steps {
            let Some(b) = g.modules.get(s.module.as_str()).filter(|b| is_input(b)) else {
                continue;
            };
            if b.channel.is_none() {
                w.sink.add(|| issue(C::InputNoChannel, [("input", cut(&s.module))]));
            }
            let is_track = |id: &str| g.tracks.contains_key(id) || w.idx.is_disabled(id);
            let Some(track) = *landing.get_or_insert_with(|| input_landing_track(c, &is_track)) else {
                continue;
            };
            match g.tracks.get(track) {
                None => {
                    let code = if w.idx.is_disabled(track) {
                        C::InputTrackDisabled
                    } else {
                        C::InputTrackMissing
                    };
                    w.sink
                        .add(|| issue(code, [("input", cut(&s.module)), ("track", cut(track))]));
                }
                Some(t) if t.kind == TrackType::Group => {
                    w.sink
                        .add(|| issue(C::InputTrackGroup, [("input", cut(&s.module)), ("track", cut(track))]));
                }
                Some(t) if t.kind == TrackType::Strudel => {
                    w.sink
                        .add(|| issue(C::InputTrackStrudel, [("input", cut(&s.module)), ("track", cut(track))]));
                }
                Some(_) => {}
            }
            // The engine assigns ONE input per track, so name both rather than let the last one loaded silently win.
            match fed_by.get(track) {
                Some(prev) => {
                    w.sink.add(|| {
                        issue(
                            C::TrackTwoInputs,
                            [("track", cut(track)), ("first", cut(prev)), ("second", cut(&s.module))],
                        )
                    });
                }
                None => {
                    fed_by.insert(track, s.module.as_str());
                }
            }
        }
    }
    // `channel` anywhere else is silently ignored, which looks like it took effect.
    for (name, m) in &g.modules {
        if !is_input(m) && m.channel.is_some() {
            w.sink.add(|| issue(C::ModuleChannelNotInput, [("module", cut(name))]));
        }
    }
}

/// Group membership: a member must exist, must not be the group itself, and must belong to only one group.
fn groups(w: &mut Walk<'_, '_>) {
    let g = w.g;
    let mut claimed: HashMap<&str, &str> = HashMap::new();
    for (id, t) in &g.tracks {
        if t.kind != TrackType::Group {
            continue;
        }
        for m in t.tracks {
            if m == id {
                w.sink.add(|| issue(C::GroupContainsItself, [("group", cut(id))]));
            } else if !g.tracks.contains_key(m.as_str()) && !w.idx.is_disabled(m) {
                w.sink
                    .add(|| issue(C::GroupTrackMissing, [("group", cut(id)), ("track", cut(m))]));
            }
            match claimed.get(m.as_str()) {
                Some(owner) => {
                    w.sink.add(|| {
                        issue(
                            C::TrackTwoGroups,
                            [("track", cut(m)), ("first", cut(owner)), ("second", cut(id))],
                        )
                    });
                }
                None => {
                    claimed.insert(m.as_str(), id);
                }
            }
        }
    }
}

/// Assembly membership: the same three questions a group's is, over any node rather than over lanes.
fn assemblies(w: &mut Walk<'_, '_>) {
    let g = w.g;
    let mut in_box: HashMap<&str, &str> = HashMap::new();
    for (id, a) in &g.assemblies {
        if g.modules.contains_key(id.as_str()) || g.tracks.contains_key(id.as_str()) {
            w.sink.add(|| issue(C::AssemblyNameTaken, [("assembly", cut(id))]));
        }
        for (member, node) in &a.members {
            let known = g.modules.contains_key(node.as_str())
                || g.tracks.contains_key(node.as_str())
                || w.idx.is_disabled(node);
            if !known {
                w.sink.add(|| {
                    issue(
                        C::AssemblyMemberMissing,
                        [("assembly", cut(id)), ("member", cut(member)), ("node", cut(node))],
                    )
                });
            }
            match in_box.get(node.as_str()) {
                Some(owner) => {
                    w.sink.add(|| {
                        issue(
                            C::NodeTwoAssemblies,
                            [("node", cut(node)), ("first", cut(owner)), ("second", cut(id))],
                        )
                    });
                }
                None => {
                    in_box.insert(node.as_str(), id.as_str());
                }
            }
        }
    }
}

/// The input labels this machine's rig does not know: one sentence each, at most `room` of them (411).
pub(super) fn input_labels(g: &GraphDoc<'_>, rig: &Rig, room: usize, more: &mut bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut index = None;
    let mut have: Option<String> = None;
    for (name, b) in &g.modules {
        let Some(Channel::Label(label)) = &b.channel else {
            continue;
        };
        if !is_input(b) {
            continue;
        }
        let known = index.get_or_insert_with(|| rig.index());
        if known.input(b.m.authored.channel.as_ref()).unknown_label.is_none() {
            continue;
        }
        if out.len() >= room.min(MAX_ISSUES) {
            *more = true;
            break;
        }
        let have = have.get_or_insert_with(|| have_list(rig));
        let tail = if have.is_empty() {
            "none are defined".to_owned()
        } else {
            format!("have: {have}")
        };
        out.push(format!(
            "input '{}': channel '{}' is not an input label in preferences.yaml preferences — {tail}",
            cut(name),
            cut(label)
        ));
    }
    out
}

/// The labels the rig has, joined, cut to 300 bytes at a character boundary and built once (411).
fn have_list(rig: &Rig) -> String {
    const CUT: usize = 300;
    let mut s = String::new();
    for (i, e) in rig.channels_in.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(&e.label);
        if s.len() >= CUT {
            break;
        }
    }
    let mut end = s.len().min(CUT);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    s.truncate(end);
    s
}
