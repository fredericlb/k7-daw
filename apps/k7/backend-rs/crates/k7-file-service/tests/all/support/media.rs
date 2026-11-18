//! The media tests' reader of `vectors/media.json`: a number written as its name ("NaN", "Infinity") is read as that f64.

use std::sync::OnceLock;

use k7_file_service::media::region::Envelope;
use serde_json::Value;
use song::clips::ClipDoc;

pub fn vectors() -> &'static Value {
    static V: OnceLock<Value> = OnceLock::new();
    V.get_or_init(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/media.json"
        ))
        .unwrap();
        serde_json::from_str(&text).unwrap()
    })
}

pub fn section(name: &str) -> &'static [Value] {
    vectors()[name]
        .as_array()
        .unwrap_or_else(|| panic!("no section {name}"))
}

/// A number as the vectors write it: a JSON number, or one of the three names a non-finite number is written as.
pub fn f(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().unwrap(),
        Value::String(s) => match s.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("not a number: {other}"),
        },
        other => panic!("not a number: {other}"),
    }
}

pub fn fs(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(f).collect()
}

pub fn same(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

/// A clip of the vectors; a non-finite number is patched in after the typed read, which has no name for it.
pub fn clip(v: &Value) -> ClipDoc {
    let mut tame = v.clone();
    let mut patches: Vec<(&str, f64)> = Vec::new();
    for k in ["start", "length", "offset", "sourceFrom", "sourceTo"] {
        if let Some(x) = v.get(k).filter(|x| x.is_string()) {
            patches.push((k, f(x)));
            tame[k] = Value::from(0.0);
        }
    }
    let mut marks: Vec<(usize, usize, f64)> = Vec::new();
    if let Some(ms) = tame
        .pointer_mut("/warp/markers")
        .and_then(Value::as_array_mut)
    {
        for (i, m) in ms.iter_mut().enumerate() {
            for (j, x) in m.as_array_mut().unwrap().iter_mut().enumerate() {
                if x.is_string() {
                    marks.push((i, j, f(x)));
                    *x = Value::from(0.0);
                }
            }
        }
    }
    let mut c: ClipDoc = serde_json::from_value(tame).unwrap();
    for (k, x) in patches {
        match k {
            "start" => c.start = x,
            "length" => c.length = x,
            "offset" => c.offset = x,
            "sourceFrom" => c.source_from = Some(x),
            _ => c.source_to = Some(x),
        }
    }
    for (i, j, x) in marks {
        let m = &mut c.warp.as_mut().unwrap().markers[i];
        if j == 0 {
            m.0 = x;
        } else {
            m.1 = x;
        }
    }
    c
}

/// A project of the vectors as the three numbers the slicing reads.
pub fn project(v: &Value) -> (f64, f64, f64) {
    (
        f(&v["tempo"]),
        f(&v["timeSigNumerator"]),
        f(&v["timeSigDenominator"]),
    )
}

/// An envelope that owns its arrays.
pub struct Env {
    pub sample_rate: f64,
    pub frames: f64,
    pub seconds: f64,
    pub peak: Vec<f64>,
    pub rms: Vec<f64>,
    pub bands: Option<[Vec<f64>; 3]>,
    pub transients: Vec<f64>,
    pub fingerprint: String,
    pub sensitivity: f64,
}

impl Env {
    pub fn of(v: &Value) -> Env {
        let band = |k: &str| v.get(k).map(fs);
        let bands = match (band("low"), band("mid"), band("high")) {
            (Some(l), Some(m), Some(h)) => Some([l, m, h]),
            _ => None,
        };
        Env {
            sample_rate: f(&v["sampleRate"]),
            frames: f(&v["frames"]),
            seconds: f(&v["seconds"]),
            peak: fs(&v["peak"]),
            rms: fs(&v["rms"]),
            bands,
            transients: fs(&v["transients"]),
            fingerprint: v["fingerprint"].as_str().unwrap().to_string(),
            sensitivity: f(&v["sensitivity"]),
        }
    }

    pub fn env(&self) -> Envelope<'_> {
        Envelope {
            sample_rate: self.sample_rate,
            frames: self.frames,
            seconds: self.seconds,
            peak: &self.peak,
            rms: &self.rms,
            bands: self
                .bands
                .as_ref()
                .map(|b| [b[0].as_slice(), b[1].as_slice(), b[2].as_slice()]),
            transients: &self.transients,
        }
    }
}

/// `expected` against `got` parsed: numbers as f64, and a non-finite name against the `null` the wire writes.
pub fn same_json(expected: &Value, got: &Value, at: &str) -> Result<(), String> {
    match (expected, got) {
        (Value::String(s), Value::Null)
            if matches!(s.as_str(), "NaN" | "Infinity" | "-Infinity") =>
        {
            Ok(())
        }
        (Value::Number(a), Value::Number(b)) if same(a.as_f64().unwrap(), b.as_f64().unwrap()) => {
            Ok(())
        }
        (Value::Array(a), Value::Array(b)) if a.len() == b.len() => a
            .iter()
            .zip(b)
            .enumerate()
            .try_for_each(|(i, (x, y))| same_json(x, y, &format!("{at}[{i}]"))),
        (Value::Object(a), Value::Object(b)) if a.len() == b.len() => {
            a.iter().try_for_each(|(k, x)| match b.get(k) {
                Some(y) => same_json(x, y, &format!("{at}.{k}")),
                None => Err(format!("{at}: no key {k}")),
            })
        }
        (a, b) if a == b => Ok(()),
        (a, b) => Err(format!("{at}: expected {a}, got {b}")),
    }
}
