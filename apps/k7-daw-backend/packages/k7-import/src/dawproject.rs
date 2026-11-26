// dawproject.rs — the DAWproject mapper: unzip in memory, read project.xml, project it onto the neutral shape.
// What a reader gets wrong: nothing here touches a disk, and a container plays a WINDOW of its children (docs/import.md).

use std::collections::HashMap;

use crate::inflate::{MAX_INFLATED_BYTES, bounded_unzip};
use crate::model::{
    Bytes, ClipKind, ClipOut, NoteOut, PluginOut, SendOut, Song, Stats, TrackOut, Translation,
    WarpOut,
};
use crate::xml::{Node, boolean, num, parse, refuse_declarations, round, round6};

const DEVICE_TAGS: [(&str, &str); 5] = [
    ("BuiltinDevice", "builtin"),
    ("Vst3Plugin", "vst3"),
    ("Vst2Plugin", "vst2"),
    ("ClapPlugin", "clap"),
    ("AuPlugin", "au"),
];

/// The three tags a clip's content can be: anything else an id names is not content.
const CONTENT_TAGS: [&str; 3] = ["Notes", "Audio", "Clips"];

/// One element the document defines under an id, and the tag it was written as.
struct Referenced<'a> {
    tag: &'a str,
    node: &'a Node,
}

struct Collect<'a> {
    track: &'a str,
    sample_rate: Option<f64>,
    warnings: Vec<String>,
    by_id: &'a HashMap<String, Referenced<'a>>,
    tempo: f64,
}

pub fn parse_dawproject(
    zip_bytes: &[u8],
    fallback: &str,
    inflate_limit: Option<usize>,
) -> Result<Translation, String> {
    let entries = bounded_unzip(zip_bytes, inflate_limit.unwrap_or(MAX_INFLATED_BYTES))?;
    let entry = |name: &str| entries.iter().find(|(n, _)| n == name).map(|(_, b)| b);
    let project_xml =
        entry("project.xml").ok_or("not a dawproject: project.xml missing from the archive")?;
    let text = String::from_utf8_lossy(project_xml).into_owned();
    refuse_declarations(&text)?;
    let project = parse(&text)?;
    let meta = match entry("metadata.xml") {
        Some(b) => Some(parse(&String::from_utf8_lossy(b))?),
        None => None,
    };

    let transport = project.child("Transport");
    let tempo = num(
        transport
            .and_then(|t| t.child("Tempo"))
            .and_then(|t| t.attr("value")),
        120.0,
    );
    let signature = transport.and_then(|t| t.child("TimeSignature"));
    let numerator = num(signature.and_then(|s| s.attr("numerator")), 4.0);
    let denominator = num(signature.and_then(|s| s.attr("denominator")), 4.0);
    let title = meta
        .as_ref()
        .and_then(|m| m.child("Title"))
        .map(|t| t.text.as_str())
        .filter(|t| !t.is_empty());
    let name = title.unwrap_or(fallback).to_owned();

    let structure: Vec<&Node> = project
        .child("Structure")
        .map(|s| s.kids("Track").collect())
        .unwrap_or_default();
    let mut channel_to_track: HashMap<&str, &str> = HashMap::new();
    for t in &structure {
        if let Some(ch) = t.child("Channel").and_then(|c| c.attr("id"))
            && let Some(id) = t.attr("id")
        {
            channel_to_track.insert(ch, id);
        }
    }

    let mut by_id = HashMap::new();
    index_by_id(&project, &mut by_id, "Project");

    let mut sample_rate = 0.0;
    let mut by_track: HashMap<&str, Vec<ClipOut>> = HashMap::new();
    let mut warnings = Vec::new();
    let (mut audio_clips, mut midi_clips, mut notes) = (0, 0, 0);
    for arrangement in project.kids("Arrangement") {
        for outer in arrangement.kids("Lanes") {
            for group in outer.kids("Lanes") {
                let Some(track) = group.attr("track") else {
                    continue;
                };
                let mut ctx = Collect {
                    track,
                    sample_rate: None,
                    warnings: Vec::new(),
                    by_id: &by_id,
                    tempo,
                };
                let mut out = Vec::new();
                let clips: Vec<&Node> = group
                    .child("Clips")
                    .map(|c| c.kids("Clip").collect())
                    .unwrap_or_default();
                collect_clips(&clips, 0.0, 0.0, None, &mut out, &mut ctx, false);
                if sample_rate == 0.0
                    && let Some(sr) = ctx.sample_rate
                {
                    sample_rate = sr;
                }
                warnings.append(&mut ctx.warnings);
                for c in &out {
                    match c.kind {
                        ClipKind::Audio => audio_clips += 1,
                        ClipKind::Midi => {
                            midi_clips += 1;
                            notes += c.notes.len();
                        }
                    }
                }
                if !out.is_empty() {
                    by_track.insert(track, out);
                }
            }
        }
    }

    let tracks: Vec<TrackOut> = structure
        .iter()
        .map(|t| {
            let channel = t.child("Channel");
            let id = t.attr("id").unwrap_or_default();
            let sends = channel
                .map(|c| {
                    c.at(&["Sends"])
                        .map(|s| s.kids("Send").collect::<Vec<_>>())
                        .unwrap_or_default()
                })
                .unwrap_or_default()
                .into_iter()
                .map(|s| {
                    let dest = s.attr("destination").unwrap_or_default();
                    SendOut {
                        to: channel_to_track.get(dest).map_or(dest, |t| t).to_owned(),
                        level: num(s.child("Volume").and_then(|v| v.attr("value")), 0.0),
                        enabled: boolean(s.child("Enable").and_then(|v| v.attr("value"))),
                    }
                })
                .collect();
            let mut track = TrackOut {
                id: id.to_owned(),
                name: t.attr("name").unwrap_or(id).to_owned(),
                role: channel.and_then(|c| c.attr("role")).map(str::to_owned),
                color: t.attr("color").map(str::to_owned),
                plugins: channel
                    .map(|c| map_devices(c.child("Devices")))
                    .unwrap_or_default(),
                sends,
                clips: by_track.remove(id).unwrap_or_default(),
                ..TrackOut::default()
            };
            if let Some(mute) = channel
                .and_then(|c| c.child("Mute"))
                .filter(|m| m.present())
            {
                track.mute = boolean(mute.attr("value"));
            }
            track
        })
        .collect();

    let assets: Vec<(String, Bytes)> = entries
        .iter()
        .filter(|(p, _)| p != "project.xml" && p != "metadata.xml" && !p.ends_with('/'))
        .cloned()
        .collect();
    let stats = Stats {
        tracks: tracks.len(),
        audio_clips,
        midi_clips,
        notes,
        assets: assets.len(),
    };
    Ok(Translation {
        song: Song {
            name,
            tempo,
            sample_rate: if sample_rate == 0.0 {
                48000.0
            } else {
                sample_rate
            },
            time_signature: format!("{numerator}/{denominator}"),
            tracks,
        },
        files: assets,
        stats,
        warnings,
    })
}

/// A Channel's Devices as plugin refs, grouped by device tag: the order across tags is not kept.
fn map_devices(devices: Option<&Node>) -> Vec<PluginOut> {
    let Some(devices) = devices else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (tag, kind) in DEVICE_TAGS {
        for d in devices.kids(tag) {
            out.push(PluginOut {
                kind: kind.to_owned(),
                name: d
                    .attr("deviceName")
                    .or_else(|| d.attr("name"))
                    .unwrap_or(kind)
                    .to_owned(),
                role: d.attr("deviceRole").map(str::to_owned),
                enabled: boolean(d.child("Enabled").and_then(|e| e.attr("value"))),
                path: d
                    .child("State")
                    .and_then(|s| s.attr("path"))
                    .map(str::to_owned),
                params: Vec::new(),
                params_complete: false,
            });
        }
    }
    out
}

/// Every element carrying an `id`, by that id: a clip's content is written ONCE and referenced.
fn index_by_id<'a>(node: &'a Node, out: &mut HashMap<String, Referenced<'a>>, tag: &'a str) {
    if let Some(id) = node.attr("id")
        && !tag.is_empty()
    {
        out.entry(id.to_owned()).or_insert(Referenced { tag, node });
    }
    for c in &node.children {
        index_by_id(c, out, &c.tag);
    }
}

/// The `<Audio>` of an audio leaf: directly under the clip, inside its `<Warps>`, or in a `<Lanes>` lane.
fn find_audio(clip: &Node) -> Option<&Node> {
    if let Some(a) = clip.child("Audio").filter(|a| a.present()) {
        return Some(a);
    }
    if let Some(a) = clip.at(&["Warps", "Audio"]).filter(|a| a.present()) {
        return Some(a);
    }
    for l in clip.kids("Lanes") {
        if let Some(a) = l
            .at(&["Warps", "Audio"])
            .or_else(|| l.child("Audio"))
            .filter(|a| a.present())
        {
            return Some(a);
        }
    }
    None
}

fn find_warps(clip: &Node) -> Option<&Node> {
    if let Some(w) = clip.child("Warps").filter(|w| w.present()) {
        return Some(w);
    }
    clip.kids("Lanes")
        .find_map(|l| l.child("Warps").filter(|w| w.present()))
}

fn warp_of(warps: Option<&Node>, audio: &Node) -> Option<WarpOut> {
    let markers: Vec<(f64, f64)> = warps?
        .kids("Warp")
        .map(|w| {
            (
                round6(num(w.attr("time"), 0.0)),
                round6(num(w.attr("contentTime"), 0.0)),
            )
        })
        .collect();
    if markers.len() < 2 {
        return None;
    }
    Some(WarpOut {
        algorithm: audio.attr("algorithm").map(str::to_owned),
        markers,
    })
}

/// The clip with its referenced content copied in, its own time, duration and loop untouched.
fn with_referenced(clip: &Node, ctx: &Collect<'_>) -> Node {
    let own = |c: &Node| {
        c.child("Notes").is_some_and(Node::present)
            || c.child("Clips").is_some_and(Node::present)
            || find_audio(c).is_some()
    };
    if own(clip) {
        return clip.clone();
    }
    let Some(target) = clip.attr("reference").and_then(|r| ctx.by_id.get(r)) else {
        return clip.clone();
    };
    if !CONTENT_TAGS.contains(&target.tag) {
        return clip.clone();
    }
    let mut out = clip.clone();
    out.children.push(target.node.clone());
    out
}

/// 0 and absent are the same here: a field at its default is left unset.
fn or_unset(v: Option<&str>) -> Option<f64> {
    let n = num(v, 0.0);
    (v.is_some() && n != 0.0).then(|| round6(n))
}

fn some_nonzero(n: f64) -> Option<f64> {
    (n != 0.0).then(|| round6(n))
}

/// A clip list flattened into absolute-positioned leaves, clamped to the window of their container.
fn collect_clips(
    clips: &[&Node],
    base_start: f64,
    base_play_start: f64,
    bounds: Option<(f64, f64)>,
    out: &mut Vec<ClipOut>,
    ctx: &mut Collect<'_>,
    parent_disabled: bool,
) {
    for raw in clips {
        let clip = with_referenced(raw, ctx);
        let time = num(clip.attr("time"), 0.0);
        let raw_start = base_start + (time - base_play_start);
        let mut start = raw_start;
        let duration = num(clip.attr("duration"), 0.0);
        let mut end = start + duration;
        if let Some((from, to)) = bounds {
            start = start.max(from);
            end = end.min(to);
        }
        let length = round6((end - start).max(0.0));
        if length <= 0.0 {
            continue;
        }
        let play_start = if clip.attr("contentTimeUnit") == Some("seconds") {
            num(clip.attr("playStart"), 0.0) * ctx.tempo / 60.0
        } else {
            num(clip.attr("playStart"), 0.0)
        };
        let offset = some_nonzero(play_start + (start - raw_start));
        let disabled = parent_disabled || clip.attr("enable") == Some("false");

        let notes_node = clip.child("Notes").filter(|n| n.present());
        let audio = find_audio(&clip);
        let nested: Vec<&Node> = clip
            .child("Clips")
            .map(|c| c.kids("Clip").collect())
            .unwrap_or_default();

        if let Some(nn) = notes_node {
            let mut c = ClipOut::new(ClipKind::Midi, round6(start), length);
            c.offset = offset;
            c.disabled = disabled;
            c.loop_start = or_unset(clip.attr("loopStart"));
            c.loop_end = or_unset(clip.attr("loopEnd"));
            c.notes = nn
                .kids("Note")
                .map(|n| NoteOut {
                    time: round6(num(n.attr("time"), 0.0)),
                    duration: round6(num(n.attr("duration"), 0.0)),
                    key: num(n.attr("key"), 0.0),
                    vel: round(num(n.attr("vel"), 1.0), 4),
                })
                .collect();
            out.push(c);
        } else if let Some(audio) = audio {
            if let Some(sr) = or_unset(audio.attr("sampleRate"))
                && ctx.sample_rate.is_none()
            {
                ctx.sample_rate = Some(sr);
            }
            let mut c = ClipOut::new(ClipKind::Audio, round6(start), length);
            c.source = audio
                .child("File")
                .and_then(|f| f.attr("path"))
                .map(str::to_owned);
            c.offset = offset;
            c.disabled = disabled;
            c.fade_in = or_unset(clip.attr("fadeInTime"));
            c.fade_out = or_unset(clip.attr("fadeOutTime"));
            c.warp = warp_of(find_warps(&clip), audio);
            out.push(c);
        } else if !nested.is_empty() {
            let bounds = Some((start, end));
            collect_clips(
                &nested,
                raw_start,
                num(clip.attr("playStart"), 0.0),
                bounds,
                out,
                ctx,
                disabled,
            );
        } else {
            ctx.warnings.push(unrecognised(&clip, start, ctx));
        }
    }
}

/// The warning for a clip of no shape this reads, naming which of three faults it is.
fn unrecognised(clip: &Node, start: f64, ctx: &Collect<'_>) -> String {
    let label = clip.attr("name").unwrap_or("(unnamed)");
    let why = match clip.attr("reference") {
        None => {
            let kids: Vec<&str> = clip.grouped().iter().map(|(t, _)| *t).collect();
            let shown = if kids.is_empty() {
                "none".to_owned()
            } else {
                kids.join(", ")
            };
            format!("no audio/notes/nested clips (children: {shown})")
        }
        Some(r) => match ctx.by_id.get(r) {
            Some(t) => format!(
                "its content points at id \"{r}\", which is a <{}> and not clip content",
                t.tag
            ),
            None => format!("its content points at id \"{r}\", which this file does not define"),
        },
    };
    format!(
        "track {}: dropped clip \"{label}\" @beat {} — {why}",
        ctx.track,
        k7x::edit::js_number(round6(start))
    )
}
