//! state_frame.rs — the ~20 Hz `state` frame as JSON, the wire's tree field for field and its rounding
//! (a peak to four decimals, a level to a hundredth of a dB; the playhead, needles and master keep full precision).
//! What a reader gets wrong: `master` is a float in the engine, widened to a double and printed whole, as
//! nlohmann did, so a frame holds a 17-digit master level beside its rounded `levels`.

use crate::engine::state::{K7Envelope, StateFrame};
use serde_json::{Map, Value, json};

fn peak(v: f32) -> f64 {
    (f64::from(v) * 1e4).round() / 1e4
}

fn level(v: f32) -> f64 {
    (f64::from(v) * 100.0).round() / 100.0
}

fn levels(db: &[f32]) -> Value {
    Value::Array(db.iter().map(|&v| json!(level(v))).collect())
}

fn envelope(points: &[K7Envelope]) -> Value {
    Value::Array(
        points
            .iter()
            .map(|e| json!({"min": peak(e.min), "max": peak(e.max)}))
            .collect(),
    )
}

/// The frame of one session, ready to print.
pub fn frame(s: &StateFrame<'_>) -> Value {
    let mut waveform = Map::new();
    for (key, points) in s.waveforms() {
        waveform.insert(key, envelope(points));
    }
    let mut recorders = Map::new();
    for r in s.recorders() {
        let takes: Vec<Value> = r
            .takes
            .iter()
            .map(|(file, seconds, damaged)| json!({"file": file, "seconds": seconds, "damaged": damaged}))
            .collect();
        recorders.insert(
            r.id,
            json!({
                "recording": r.recording,
                "seconds": r.seconds,
                "takes": takes,
                "envelope": envelope(r.envelope),
                // Where those points START in the take: an append, or a resend already drawn.
                "from": r.envelope_from,
            }),
        );
    }
    let platters: Map<String, Value> = s
        .platters()
        .into_iter()
        .map(|(id, seconds)| (id, json!(seconds)))
        .collect();
    let mut out = json!({
        "type": "state",
        "session": s.session(),
        "playing": s.playing(),
        "playhead": s.playhead(),
        "loop": {"on": s.loop_on(), "start": s.loop_start(), "end": s.loop_end()},
        "levels": levels(s.levels()),
        "inputLevels": levels(s.input_levels()),
        "armed": s.armed(),
        "recordEnabled": s.record_enabled(),
        "master": f64::from(s.master_db()),
        "masterClipped": s.master_clipped(),
        "waveform": waveform,
        "platters": platters,
        "recorders": recorders,
    });
    // The cost report, on the frames that carry one: absent otherwise, and the client keeps the last.
    if let Some(load) = s.load() {
        let mut plugins = Map::new();
        for (id, cpu, bytes) in load.plugins {
            let mut p = json!({"cpu": cpu});
            if bytes >= 0 {
                p["bytes"] = json!(bytes);
            }
            plugins.insert(id, p);
        }
        let mut report = json!({"plugins": plugins});
        if load.process_bytes >= 0 {
            report["process"] = json!(load.process_bytes);
        }
        out["load"] = report;
    }
    out
}
