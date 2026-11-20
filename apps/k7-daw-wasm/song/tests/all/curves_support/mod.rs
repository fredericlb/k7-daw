// curves_support/mod.rs — what the curve tests share: a curve file decoded from k7x text, a time signature, a budget.
// A reader would get wrong that this holds no Source: a test that reads a folder uses support/mod.rs beside it.

#![allow(dead_code)]

use serde_json::Value;
use song::compare::{Rules, compare};
use song::curves::{
    BUDGET, BakedCurve, Budget, CurveFile, EnvelopeFire, ResolvedCurve, bake_adsr, bake_curve, envelope_fires,
};
use song::{FileValue, Finding};
use song_schema::TimeSig;

/// A curve file decoded as the compile decodes it.
pub fn decode(text: &str) -> Result<CurveFile, Finding> {
    CurveFile::decode("curves/x.k7x", text, song::PathRule::HOST)
}

/// A curve file that must decode.
pub fn file(text: &str) -> CurveFile {
    decode(text).unwrap_or_else(|f| panic!("{text}: {}", f.message))
}

pub fn sig(num: u32, den: u32) -> TimeSig {
    TimeSig::new(num, den).expect("a signature")
}

/// A curve file resolved against a signature.
pub fn resolved(text: &str, sig: TimeSig) -> ResolvedCurve {
    ResolvedCurve::new(&file(text), sig).expect("resolves")
}

/// 4/4: a bar is 4 beats.
pub fn r44(text: &str) -> ResolvedCurve {
    resolved(text, sig(4, 4))
}

pub fn budget() -> Budget {
    Budget::new(BUDGET)
}

/// A firing that retriggers and has no window.
pub fn fire(at: f64) -> EnvelopeFire {
    EnvelopeFire {
        at,
        stop: None,
        retrigger: true,
    }
}

/// Firings at `ats` with one retrigger rule and no window: the TypeScript's bare-number triggers and `opts`.
pub fn fires(ats: &[f64], retrigger: bool) -> Vec<EnvelopeFire> {
    ats.iter()
        .map(|at| EnvelopeFire {
            at: *at,
            stop: None,
            retrigger,
        })
        .collect()
}

/// bakeCurve in 4/4 over a song of `song_beats`.
pub fn bake44(text: &str, song_beats: f64, triggers: &[f64]) -> BakedCurve {
    bake_curve(&r44(text), song_beats, triggers, &mut budget()).expect("bakes")
}

/// bakeAdsr at `tempo` over the firings of bare beats.
pub fn adsr(text: &str, ats: &[f64], tempo: f64) -> BakedCurve {
    bake_adsr(&r44(text), &fires(ats, true), tempo, &mut budget()).expect("bakes")
}

/// An envelope's firings under its own conditions, in 4/4.
pub fn fires_under(c: &ResolvedCurve, note_ons: &[f64]) -> Vec<EnvelopeFire> {
    envelope_fires(c, note_ons, &mut budget()).expect("fires")
}

/// Whether `actual` is `expected` (JSON text) with numbers within 1e-9.
pub fn same(actual: &impl serde::Serialize, expected: &str) -> bool {
    let a = serde_json::to_value(actual).expect("json");
    let e: Value = serde_json::from_str(expected).expect("expected json");
    compare(&a, &e, &Rules::default()).is_empty()
}

pub fn json_of(f: &CurveFile) -> Value {
    serde_json::to_value(f).expect("json")
}

/// The points of a bake as plain pairs.
pub fn pairs(c: &BakedCurve) -> Vec<(f64, f64)> {
    c.points.iter().map(|p| (p[0], p[1])).collect()
}
