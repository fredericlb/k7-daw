// als_devices.rs — a Live track's devices as plugin refs: racks flattened away, Live's own devices reported and skipped.
// What a reader gets wrong: Live names no role, so the first plugin on a MIDI track is taken as its instrument (docs/import.md).

use indexmap::IndexSet;

use crate::model::{Bytes, PluginOut};
use crate::xml::{Node, num, round6, val};

/// What a device walk collects besides the plugins: names reported once in the log, and the asset names taken.
#[derive(Default)]
pub struct Devices {
    pub native: IndexSet<String>,
    pub racks: IndexSet<String>,
    pub used_names: std::collections::HashSet<String>,
    pub files: Vec<(String, Bytes)>,
}

const NATIVE_NAMES: [&str; 14] = [
    "Eq8",
    "Compressor2",
    "Reverb",
    "Operator",
    "Wavetable",
    "AutoFilter",
    "Delay",
    "Saturator",
    "Limiter",
    "GlueCompressor",
    "MultibandDynamics",
    "Echo",
    "Roar",
    "Drift",
];

fn is_native(tag: &str) -> bool {
    tag.ends_with("Device") || NATIVE_NAMES.contains(&tag) || tag == "Meld"
}

fn not_a_device(tag: &str) -> bool {
    tag == "MixerDevice"
        || tag.ends_with("DeviceChain")
        || tag == "DevicesListWrapper"
        || tag == "Devices"
}

/// `name` made unique among the asset names taken, as `foo.wav`, `foo-2.wav`.
pub fn unique_name(name: &str, used: &mut std::collections::HashSet<String>) -> String {
    let dot = name.rfind('.').filter(|d| *d > 0);
    let (stem, ext) = match dot {
        Some(d) => (&name[..d], &name[d..]),
        None => (name, ""),
    };
    let mut candidate = name.to_owned();
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = format!("{stem}-{n}{ext}");
        n += 1;
    }
    used.insert(candidate.clone());
    candidate
}

/// Live's `<Buffer>` hex is wrapped across lines and indented: everything non-hex goes before decoding.
fn decode_hex(text: &str) -> Option<Vec<u8>> {
    let clean: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
    if clean.len() < 2 {
        return None;
    }
    let digit = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
    Some(
        clean
            .as_chunks::<2>()
            .0
            .iter()
            .map(|[hi, lo]| digit(*hi) << 4 | digit(*lo))
            .collect(),
    )
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut pending = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            if pending && !out.is_empty() {
                out.push('-');
            }
            pending = false;
            out.push(c);
        } else {
            pending = true;
        }
    }
    if out.is_empty() { "plugin".into() } else { out }
}

fn walk<'a>(node: &'a Node, found: &mut Vec<(&'a str, &'a Node)>, acc: &mut Devices) {
    for (tag, kids) in node.grouped() {
        if tag == "PluginDevice" || tag == "AuPluginDevice" {
            found.extend(kids.into_iter().map(|d| (tag, d)));
            continue;
        }
        if !not_a_device(tag) && is_native(tag) {
            let into = if tag.ends_with("GroupDevice") {
                &mut acc.racks
            } else {
                &mut acc.native
            };
            into.insert(tag.to_owned());
        }
        for k in kids {
            walk(k, found, acc);
        }
    }
}

/// A track's Devices as plugin refs; `notes` is true for a MIDI track, whose first plugin is its instrument.
pub fn map_devices(devices: Option<&Node>, notes: bool, acc: &mut Devices) -> Vec<PluginOut> {
    let Some(devices) = devices else {
        return Vec::new();
    };
    let mut found = Vec::new();
    walk(devices, &mut found, acc);
    let mut out: Vec<PluginOut> = Vec::new();
    for (tag, d) in found {
        let desc = d.child("PluginDesc");
        let vst3 = desc.and_then(|p| p.child("Vst3PluginInfo"));
        let vst2 = desc.and_then(|p| p.child("VstPluginInfo"));
        let info = vst3.or(vst2);
        let name = val(info, &["Name"])
            .or_else(|| val(info, &["PlugName"]))
            .unwrap_or("unknown plugin")
            .to_owned();
        let kind = if tag == "AuPluginDevice" {
            "au"
        } else if vst3.is_some() {
            "vst3"
        } else {
            "vst2"
        };
        let on = val(Some(d), &["On", "Manual"]);
        let mut plugin = PluginOut {
            kind: kind.to_owned(),
            name: name.clone(),
            role: None,
            enabled: on.is_none_or(|v| v == "true"),
            path: None,
            params: Vec::new(),
            params_complete: false,
        };
        let raw = vst3
            .and_then(|v| v.at(&["Preset", "Vst3Preset", "ProcessorState"]))
            .map(|n| n.text.as_str())
            .filter(|t| !t.is_empty())
            .or_else(|| {
                vst2.and_then(|v| v.at(&["Preset", "VstPreset", "Buffer"]))
                    .map(|n| n.text.as_str())
                    .filter(|t| !t.is_empty())
            });
        if let Some(bytes) = raw.and_then(decode_hex) {
            let key = format!(
                "plugins/{}",
                unique_name(&format!("{}.vstpreset", slug(&name)), &mut acc.used_names)
            );
            acc.files.push((key.clone(), Bytes::new(bytes)));
            plugin.path = Some(key);
        }
        if let Some(list) = d.child("ParameterList") {
            for p in list.kids("PluginFloatParameter") {
                let pname = val(Some(p), &["ParameterName"]).filter(|s| !s.is_empty());
                let pval = val(Some(p), &["ParameterValue", "Manual"]);
                if let (Some(n), Some(v)) = (pname, pval) {
                    plugin
                        .params
                        .push((n.to_owned(), round6(num(Some(v), 0.0))));
                }
            }
        }
        plugin.role = Some(
            if notes && out.is_empty() {
                "instrument"
            } else {
                "audioFX"
            }
            .into(),
        );
        out.push(plugin);
    }
    out
}
