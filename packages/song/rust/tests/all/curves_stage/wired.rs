// curves_stage/wired.rs — the wired half of the curves gate: bakeAutomations' lanes, redone as the lane builder will do them.
// A reader would get wrong that the binding loop is written out here: note_ons once per lane, then bake per binding.

use super::curves_support::sig;
use super::{Gate, read_json};
use indexmap::IndexMap;
use serde_json::{Value, json};
use song::curves::{BakedCurve, CurveBaker, CurveFile, MidiWindow, read_curves};
use song::{Cache, DiskSource};
use std::path::Path;

fn windows_of(track: &Value) -> Vec<(f64, f64, f64, f64, Vec<f64>)> {
    track["clips"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| c["type"] == "midi")
        .map(|c| {
            let num = |k: &str| c[k].as_f64().unwrap_or(0.0);
            let notes = c["notes"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| n["time"].as_f64())
                .collect();
            (num("start"), num("length"), num("loopStart"), num("loopEnd"), notes)
        })
        .collect()
}

pub fn wired(gate: &mut Gate, dumps: &Path) {
    let dump = read_json(&dumps.join("curves/wired.json"));
    let project = super::support::spell(&dumps.join("curves/wired"));
    let disk = DiskSource::new(std::slice::from_ref(&project)).expect("source");
    let read = read_curves(&disk, &mut Cache::new(), &project).expect("the wired curves read");
    let curves: &IndexMap<String, std::sync::Arc<CurveFile>> = &read.curves;
    for (i, case) in dump["cases"].as_array().expect("cases").iter().enumerate() {
        let name = format!("wired/{i}");
        let (num, den) = (
            case["sig"][0].as_u64().expect("num"),
            case["sig"][1].as_u64().expect("den"),
        );
        let sg = sig(u32::try_from(num).expect("num"), u32::try_from(den).expect("den"));
        let tracks = case["tracks"].as_array().expect("tracks");
        // The furthest clip end, as bakeAutomations reduces it.
        let song = tracks
            .iter()
            .flat_map(|t| t["clips"].as_array().into_iter().flatten())
            .fold(0.0_f64, |m, c| {
                m.max(c["start"].as_f64().unwrap_or(0.0) + c["length"].as_f64().unwrap_or(0.0))
            });
        let mut lane_of: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for chain in case["chains"].as_object().expect("chains").values() {
            let owner = chain["owner"].as_str().expect("owner");
            for step in chain["use"].as_array().into_iter().flatten() {
                lane_of.insert(step["module"].as_str().expect("module").to_owned(), owner.to_owned());
            }
        }
        let mut baker = CurveBaker::new(curves, sg, case["tempo"].as_f64().expect("tempo"), song);
        let mut notes: std::collections::HashMap<String, Vec<f64>> = std::collections::HashMap::new();
        let mut lanes: IndexMap<String, Vec<Value>> = tracks
            .iter()
            .map(|t| (t["id"].as_str().expect("id").to_owned(), Vec::new()))
            .collect();
        for b in case["bindings"].as_array().expect("bindings") {
            let owner = b["owner"].as_str().expect("owner");
            if !tracks.iter().any(|t| t["id"] == owner) {
                continue;
            }
            let lane = lane_of
                .get(b["trigger"].as_str().expect("trigger"))
                .map_or(owner, String::as_str);
            let events = notes.entry(lane.to_owned()).or_insert_with(|| {
                let Some(source) = tracks.iter().find(|t| t["id"] == lane) else {
                    return Vec::new();
                };
                let held = windows_of(source);
                let windows: Vec<MidiWindow> = held
                    .iter()
                    .map(|w| MidiWindow {
                        start: w.0,
                        length: w.1,
                        loop_start: w.2,
                        loop_end: w.3,
                        note_times: &w.4,
                    })
                    .collect();
                baker.note_ons(&windows)
            });
            if let Some(baked) = baker.bake(b["curve"].as_str().expect("curve"), events) {
                lanes.entry(owner.to_owned()).or_default().push(json!({
                    "param": b["param"], "pluginId": b["pluginId"], "bipolar": baked.bipolar, "points": baked.points,
                }));
            }
            gate.n.wired_bindings += 1;
        }
        let (findings, spent) = baker.finish();
        if !findings.is_empty() {
            gate.failures.push(format!("{name}: {findings:?}"));
        }
        gate.n.spent_max = gate.n.spent_max.max(spent);
        for (id, ours) in &lanes {
            let want = case["out"][id].as_array().cloned().unwrap_or_default();
            if want.len() != ours.len() {
                gate.failures.push(format!(
                    "{name} {id}: {} lanes, the TypeScript's {}",
                    ours.len(),
                    want.len()
                ));
                continue;
            }
            for (t, o) in want.iter().zip(ours) {
                if t["param"] != o["param"] || t["pluginId"] != o["pluginId"] {
                    gate.failures.push(format!("{name} {id}: a lane is for another param"));
                }
                let theirs = json!({"bipolar": t["bipolar"], "points": t["points"]});
                let baked = BakedCurve {
                    bipolar: o["bipolar"].as_bool().unwrap_or(false),
                    points: o["points"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|p| [p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0)])
                        .collect(),
                };
                gate.baked(&format!("{name} {id}"), &theirs, &baked);
                gate.n.wired += 1;
            }
        }
    }
}
