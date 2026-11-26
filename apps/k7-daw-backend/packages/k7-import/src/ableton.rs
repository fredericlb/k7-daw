// ableton.rs — the Ableton Live (.als) mapper: gunzip in memory, read the LiveSet, project it onto the neutral shape.
// What a reader gets wrong: a .als embeds no audio, so sample bytes come from the caller's reader, which owns the path policy.

use std::collections::BTreeSet;

use crate::als_devices::{Devices, map_devices, unique_name};
use crate::inflate::{MAX_INFLATED_BYTES, bounded_gunzip};
use crate::model::{
    Bytes, ClipKind, ClipOut, NoteOut, Song, Stats, TrackOut, Translation, WarpOut,
};
use crate::xml::{Node, boolean, num, parse, round6, val};

/// What a `<SampleRef><FileRef>` gave, before policy: `relative` when Live set one, `absolute` always.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SampleRef {
    pub name: String,
    pub relative: Option<String>,
    pub absolute: Option<String>,
}

/// Resolves a sample reference to its bytes, none when unreachable.
pub type ReadSample<'a> = &'a dyn Fn(&SampleRef) -> Option<Bytes>;

/// Live packs the time signature into one integer: 201 is 4/4.
#[must_use]
pub fn decode_time_signature(v: f64) -> String {
    if !v.is_finite() || v < 0.0 {
        return "4/4".into();
    }
    let numerator = v % 99.0 + 1.0;
    let denominator = 2f64.powf((v / 99.0).floor());
    format!("{numerator}/{denominator}")
}

pub fn parse_als(
    bytes: &[u8],
    fallback: &str,
    read: Option<ReadSample<'_>>,
    inflate_limit: Option<usize>,
) -> Result<Translation, String> {
    if bytes.len() < 2 || bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(
            "not an Ableton Live set: the file is not gzip-compressed (an .als always is)".into(),
        );
    }
    let xml = bounded_gunzip(bytes, inflate_limit.unwrap_or(MAX_INFLATED_BYTES))
        .map_err(|e| format!("could not decompress the .als: {e}"))?;
    map_live_set(&String::from_utf8_lossy(&xml), fallback, read)
}

struct Ctx<'a> {
    read: Option<ReadSample<'a>>,
    devices: Devices,
    tempo: f64,
    audio_clips: usize,
    midi_clips: usize,
    notes: usize,
    session_clips: usize,
    take_lane_clips: usize,
    sample_rate: f64,
    missing: BTreeSet<String>,
    ordered_missing: Vec<String>,
    files: Vec<(String, Bytes)>,
}

/// Split from `parse_als` so the mapping is testable on hand-written XML.
pub fn map_live_set(
    xml: &str,
    fallback: &str,
    read: Option<ReadSample<'_>>,
) -> Result<Translation, String> {
    let root = parse(xml)?;
    if root.tag != "Ableton" {
        return Err("not an Ableton Live set: no <Ableton> root element".into());
    }
    let creator = root.attr("Creator").unwrap_or("unknown");
    let major = num(
        root.attr("MinorVersion").and_then(|m| m.split('.').next()),
        0.0,
    );
    if major != 11.0 && major != 12.0 {
        return Err(format!(
            "unsupported Ableton version: \"{creator}\" — this importer handles Live 11 and 12"
        ));
    }
    let empty = Node::default();
    let live = root.child("LiveSet").unwrap_or(&empty);
    let main = live
        .child("MainTrack")
        .or_else(|| live.child("MasterTrack"));
    let mixer = main.and_then(|m| m.at(&["DeviceChain", "Mixer"]));
    let tempo = round6(num(val(mixer, &["Tempo", "Manual"]), 120.0));
    let time_signature =
        decode_time_signature(num(val(mixer, &["TimeSignature", "Manual"]), 201.0));

    let mut ctx = Ctx {
        read,
        devices: Devices::default(),
        tempo,
        audio_clips: 0,
        midi_clips: 0,
        notes: 0,
        session_clips: 0,
        take_lane_clips: 0,
        sample_rate: 0.0,
        missing: BTreeSet::new(),
        ordered_missing: Vec::new(),
        files: Vec::new(),
    };
    let src = live.child("Tracks").unwrap_or(&empty);
    let mut tracks = Vec::new();
    for t in src.kids("MidiTrack") {
        tracks.push(map_track(t, true, &mut ctx));
    }
    for t in src.kids("AudioTrack") {
        tracks.push(map_track(t, false, &mut ctx));
    }

    let mut warnings = Vec::new();
    let returns = src.kids("ReturnTrack").count();
    if returns > 0 {
        warnings.push(format!(
            "{returns} return track(s) ignored — sends are not imported yet"
        ));
    }
    let groups = src.kids("GroupTrack").count();
    if groups > 0 {
        warnings.push(format!(
            "{groups} group track(s) flattened — grouping is not imported yet"
        ));
    }
    if ctx.take_lane_clips > 0 {
        warnings.push(format!(
            "{} take-lane clip(s) ignored — only the takes comped into the Arrangement are imported",
            ctx.take_lane_clips
        ));
    }
    if ctx.session_clips > 0 {
        warnings.push(format!(
            "{} Session view clip(s) ignored — only the Arrangement is imported",
            ctx.session_clips
        ));
    }
    for d in &ctx.devices.native {
        warnings.push(format!(
            "Ableton device \"{d}\" skipped — it has no portable form outside Live"
        ));
    }
    for r in &ctx.devices.racks {
        warnings.push(format!(
            "rack \"{r}\" flattened — the plugins inside are carried, its structure is not"
        ));
    }
    for s in &ctx.ordered_missing {
        warnings.push(format!(
            "sample not imported: {s} — the clips playing it are left out"
        ));
    }

    let mut files = std::mem::take(&mut ctx.files);
    files.append(&mut ctx.devices.files);
    let stats = Stats {
        tracks: tracks.len(),
        audio_clips: ctx.audio_clips,
        midi_clips: ctx.midi_clips,
        notes: ctx.notes,
        assets: files.len(),
    };
    Ok(Translation {
        song: Song {
            name: fallback.to_owned(),
            tempo,
            sample_rate: if ctx.sample_rate > 0.0 {
                ctx.sample_rate
            } else {
                48000.0
            },
            time_signature,
            tracks,
        },
        files,
        stats,
        warnings,
    })
}

fn map_track(t: &Node, notes: bool, ctx: &mut Ctx<'_>) -> TrackOut {
    let id = t.attr("Id").unwrap_or_default();
    let name = val(Some(t), &["Name", "EffectiveName"])
        .filter(|n| !n.is_empty())
        .map_or_else(|| format!("track {id}"), str::to_owned);
    let mixer = t.at(&["DeviceChain", "Mixer"]);
    let mut track = TrackOut {
        id: id.to_owned(),
        name,
        role: Some("regular".into()),
        ..TrackOut::default()
    };
    // Speaker is the track's ON state: true means audible, so mute is its inverse.
    if let Some(speaker) = val(mixer, &["Speaker", "Manual"]) {
        track.mute = speaker != "true";
    }
    track.clips = collect_arrangement_clips(t, ctx);
    track.drums = t
        .child("DeviceChain")
        .is_some_and(|d| d.holds("DrumGroupDevice"));
    let devices = t.at(&["DeviceChain", "DeviceChain", "Devices"]);
    track.plugins = map_devices(devices, notes, &mut ctx.devices);
    track
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Where {
    Arrangement,
    Session,
    Takes,
}

/// Where Live keeps clips moved between versions and track kinds, so the track is walked rather than hard-coded.
fn collect_arrangement_clips(track: &Node, ctx: &mut Ctx<'_>) -> Vec<ClipOut> {
    let mut out = Vec::new();
    walk_clips(track, Where::Arrangement, &mut out, ctx);
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

fn walk_clips(node: &Node, place: Where, out: &mut Vec<ClipOut>, ctx: &mut Ctx<'_>) {
    for (key, children) in node.grouped() {
        let at = match (place, key) {
            (Where::Arrangement, "ClipSlotList" | "ClipSlot") => Where::Session,
            (Where::Arrangement, "TakeLanes") => Where::Takes,
            _ => place,
        };
        if key == "AudioClip" || key == "MidiClip" {
            for clip in children {
                match at {
                    Where::Session => ctx.session_clips += 1,
                    Where::Takes => ctx.take_lane_clips += 1,
                    Where::Arrangement => {
                        let mapped = if key == "AudioClip" {
                            map_audio_clip(clip, ctx)
                        } else {
                            Some(map_midi_clip(clip, ctx))
                        };
                        out.extend(mapped);
                    }
                }
            }
            continue;
        }
        for c in children {
            walk_clips(c, at, out, ctx);
        }
    }
}

/// The arrangement placement both clip kinds share: CurrentStart and CurrentEnd are absolute beats.
fn clip_window(clip: &Node) -> (f64, f64) {
    let start = num(
        val(Some(clip), &["CurrentStart"]),
        num(clip.attr("Time"), 0.0),
    );
    let end = num(val(Some(clip), &["CurrentEnd"]), start);
    (round6(start), round6((end - start).max(0.0)))
}

/// Where the clip starts reading its material: `<Loop>` holds two different things by the brace.
fn material_start(clip: &Node) -> f64 {
    let looped = Some(clip);
    if boolean(val(looped, &["Loop", "LoopOn"])) {
        num(val(looped, &["Loop", "StartRelative"]), 0.0)
    } else {
        num(val(looped, &["Loop", "LoopStart"]), 0.0)
    }
}

/// The source second a clip beat reads, through its warp markers: beyond either end, that end's own rate.
#[must_use]
pub fn source_second_at(markers: &[(f64, f64)], beat: f64) -> f64 {
    let seg = |i: usize| {
        let (b0, s0) = markers[i];
        let (b1, s1) = markers[i + 1];
        if b1 > b0 {
            s0 + (beat - b0) * (s1 - s0) / (b1 - b0)
        } else {
            s0
        }
    };
    for i in 0..markers.len() - 1 {
        if beat <= markers[i + 1].0 {
            return seg(i).max(0.0);
        }
    }
    seg(markers.len() - 2).max(0.0)
}

/// Only a `<SampleRef>`'s FileRef is audio: a bare one points at a device preset or a VST binary.
fn read_sample_ref(sample_ref: Option<&Node>) -> Option<SampleRef> {
    let file_ref = sample_ref?.child("FileRef")?;
    let relative = val(Some(file_ref), &["RelativePath"]).filter(|s| !s.is_empty());
    let absolute = val(Some(file_ref), &["Path"]).filter(|s| !s.is_empty());
    let name = val(Some(file_ref), &["Name"])
        .filter(|s| !s.is_empty())
        .or_else(|| {
            relative
                .or(absolute)
                .and_then(|p| p.rsplit(['/', '\\']).next())
        })
        .unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    Some(SampleRef {
        name: name.to_owned(),
        relative: relative.map(str::to_owned),
        absolute: absolute.map(str::to_owned),
    })
}

fn map_audio_clip(clip: &Node, ctx: &mut Ctx<'_>) -> Option<ClipOut> {
    let (start, length) = clip_window(clip);
    let sample_ref = clip.child("SampleRef");
    let reference = read_sample_ref(sample_ref);
    let mut out = ClipOut::new(ClipKind::Audio, start, length);
    let sr = num(val(sample_ref, &["DefaultSampleRate"]), 0.0);
    if sr > 0.0 && ctx.sample_rate == 0.0 {
        ctx.sample_rate = sr;
    }
    if let Some(r) = reference {
        let bytes = ctx.read.and_then(|f| f(&r));
        match bytes {
            Some(bytes) => {
                let key = format!(
                    "audio/{}",
                    unique_name(&r.name, &mut ctx.devices.used_names)
                );
                ctx.files.push((key.clone(), bytes));
                out.source = Some(key);
            }
            None => {
                // Left out rather than placed on a file that is not there: one such clip would refuse the whole song.
                let shown = r
                    .relative
                    .clone()
                    .or(r.absolute.clone())
                    .unwrap_or(r.name.clone());
                if ctx.missing.insert(shown.clone()) {
                    ctx.ordered_missing.push(shown);
                }
                return None;
            }
        }
    }
    let warped = boolean(val(Some(clip), &["IsWarped"]));
    let tempo = ctx.tempo;
    let to_beats = |v: f64| if warped { v } else { v * tempo / 60.0 };
    let mut markers: Vec<(f64, f64)> = if warped {
        clip.at(&["WarpMarkers"])
            .map(|w| {
                w.kids("WarpMarker")
                    .map(|m| (num(m.attr("BeatTime"), 0.0), num(m.attr("SecTime"), 0.0)))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    markers.sort_by(|a, b| a.0.total_cmp(&b.0));
    if markers.len() >= 2 {
        out.warp = Some(WarpOut {
            algorithm: None,
            markers: markers
                .iter()
                .map(|(b, s)| (round6(*b), round6(*s)))
                .collect(),
        });
    }
    let at = material_start(clip);
    let offset = if markers.len() >= 2 {
        source_second_at(&markers, at) * tempo / 60.0
    } else {
        to_beats(at)
    };
    if offset > 1e-6 {
        out.offset = Some(round6(offset));
    }
    let fade_in = num(val(Some(clip), &["Fades", "FadeInLength"]), 0.0);
    if fade_in > 0.0 {
        out.fade_in = Some(round6(to_beats(fade_in)));
    }
    let fade_out = num(val(Some(clip), &["Fades", "FadeOutLength"]), 0.0);
    if fade_out > 0.0 {
        out.fade_out = Some(round6(to_beats(fade_out)));
    }
    ctx.audio_clips += 1;
    Some(out)
}

fn map_midi_clip(clip: &Node, ctx: &mut Ctx<'_>) -> ClipOut {
    let (start, length) = clip_window(clip);
    let mut out = ClipOut::new(ClipKind::Midi, start, length);
    // MIDI is always in beats, and the loop brace is deliberately not carried.
    let offset = material_start(clip);
    if offset != 0.0 {
        out.offset = Some(round6(offset));
    }
    let mut notes: Vec<NoteOut> = Vec::new();
    let tracks = clip.at(&["Notes", "KeyTracks"]);
    for kt in tracks.into_iter().flat_map(|k| k.kids("KeyTrack")) {
        let key = num(val(Some(kt), &["MidiKey"]), -1.0);
        if key < 0.0 {
            continue;
        }
        for ev in kt
            .child("Notes")
            .into_iter()
            .flat_map(|n| n.kids("MidiNoteEvent"))
        {
            notes.push(NoteOut {
                time: round6(num(ev.attr("Time"), 0.0)),
                duration: round6(num(ev.attr("Duration"), 0.0)),
                key,
                vel: round6(num(ev.attr("Velocity"), 100.0) / 127.0),
            });
        }
    }
    notes.sort_by(|a, b| a.time.total_cmp(&b.time).then(a.key.total_cmp(&b.key)));
    ctx.notes += notes.len();
    ctx.midi_clips += 1;
    out.notes = notes;
    out
}
