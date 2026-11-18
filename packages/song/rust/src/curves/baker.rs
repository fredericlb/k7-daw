// curves/baker.rs — the bake of one compile: each curve resolved once, the lanes' note-ons laid, one budget, the errors it gathers.
// A reader would get wrong that a refusal is not an Err: the baker answers None for that lane and holds an ERROR for `finish`.

use super::bake::{BakedCurve, bake_adsr, bake_curve};
use super::conditions::envelope_fires;
use super::file::CurveFile;
use super::resolved::ResolvedCurve;
use super::{BUDGET, Budget, Over};
use crate::files::{Finding, Severity};
use indexmap::IndexMap;
use song_schema::{TimeSig, round_beat};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// One MIDI clip as the bake reads it: five numbers and the times of its notes in the clip.
#[derive(Clone, Copy, Debug)]
pub struct MidiWindow<'a> {
    pub start: f64,
    pub length: f64,
    pub loop_start: f64,
    pub loop_end: f64,
    pub note_times: &'a [f64],
}

/// The bake of one compile over the curves of its project, one `TimeSig`, one tempo and one song length.
pub struct CurveBaker<'a> {
    curves: &'a IndexMap<String, Arc<CurveFile>>,
    sig: TimeSig,
    tempo: f64,
    song_beats: f64,
    // A curve resolved once, a refusal too; not serialized, and the answer does not depend on the order asked.
    memo: HashMap<String, Option<Arc<ResolvedCurve>>>,
    budget: Budget,
    findings: Vec<Finding>,
    reported: HashSet<String>,
    capped: bool,
}

fn error(file: String, message: &str) -> Finding {
    Finding {
        file,
        line: 1,
        col: 1,
        message: message.to_owned(),
        severity: Severity::Error,
    }
}

fn file_of(id: &str) -> String {
    format!("curves/{id}.k7x")
}

fn by_beat(a: &f64, b: &f64) -> std::cmp::Ordering {
    (a + 0.0).total_cmp(&(b + 0.0))
}

impl<'a> CurveBaker<'a> {
    pub fn new(
        curves: &'a IndexMap<String, Arc<CurveFile>>,
        sig: TimeSig,
        tempo: f64,
        song_beats: f64,
    ) -> CurveBaker<'a> {
        CurveBaker {
            curves,
            sig,
            tempo,
            song_beats,
            memo: HashMap::new(),
            budget: Budget::new(BUDGET),
            findings: Vec::new(),
            reported: HashSet::new(),
            capped: false,
        }
    }

    /// One ERROR per file, the first only: a curve bound a hundred thousand times says it once.
    fn report(&mut self, file: String, message: &str) {
        if self.reported.insert(file.clone()) {
            self.findings.push(error(file, message));
        }
    }

    fn cap(&mut self, file: String) {
        self.capped = true;
        self.report(file, "the bake wrote more than it may, so it stopped here");
    }

    /// Every note-on a lane plays, in song beats, sorted, rounded and without repeats.
    pub fn note_ons(&mut self, clips: &[MidiWindow<'_>]) -> Vec<f64> {
        if self.capped {
            return Vec::new();
        }
        match self.lay(clips) {
            Ok(mut out) => {
                out.sort_by(by_beat);
                out.dedup();
                out
            }
            Err(Over) => {
                self.cap("curves".to_owned());
                Vec::new()
            }
        }
    }

    fn lay(&mut self, clips: &[MidiWindow<'_>]) -> Result<Vec<f64>, Over> {
        let mut out: Vec<f64> = Vec::new();
        for c in clips {
            let looped = if c.loop_end > c.loop_start {
                c.loop_end - c.loop_start
            } else {
                0.0
            };
            let span = if c.length > 0.0 { c.length } else { f64::INFINITY };
            // The walk of a window's notes is paid whether or not it lays any.
            self.budget.spend(c.note_times.len())?;
            for time in c.note_times {
                if looped > 0.0 {
                    if *time < c.loop_start || *time >= c.loop_end {
                        continue;
                    }
                    let mut k = 0.0_f64;
                    while k * looped < span {
                        // Each repeat is weighed: a loop of nothing in a length of nothing never ends.
                        self.budget.spend(1)?;
                        let at = time - c.loop_start + k * looped;
                        if at >= span {
                            break;
                        }
                        out.push(round_beat(c.start + at));
                        k += 1.0;
                    }
                } else if *time < span {
                    self.budget.spend(1)?;
                    out.push(round_beat(c.start + time));
                }
            }
        }
        Ok(out)
    }

    /// The curve resolved against the song, once; None when there is no such curve or it cannot be resolved.
    fn resolve(&mut self, id: &str) -> Option<Arc<ResolvedCurve>> {
        if let Some(held) = self.memo.get(id) {
            return held.clone();
        }
        let file = self.curves.get(id)?;
        let resolved = match ResolvedCurve::new(file, self.sig) {
            Ok(r) => Some(Arc::new(r)),
            Err(_) => {
                self.report(file_of(id), "a time in the curve is not a finite number of beats");
                None
            }
        };
        self.memo.insert(id.to_owned(), resolved.clone());
        resolved
    }

    /// The lane a curve makes from the note-ons of its node; None when there is no point to play.
    pub fn bake(&mut self, curve_id: &str, events: &[f64]) -> Option<BakedCurve> {
        if self.capped {
            return None;
        }
        let c = self.resolve(curve_id)?;
        // An envelope is placed by its events; every other curve is a function of song time, both under their conditions.
        let baked = if c.is_envelope() {
            envelope_fires(&c, events, &mut self.budget)
                .and_then(|fires| bake_adsr(&c, &fires, self.tempo, &mut self.budget))
        } else {
            bake_curve(&c, self.song_beats, events, &mut self.budget)
        };
        let baked = match baked {
            Ok(b) => b,
            Err(Over) => {
                self.cap(file_of(curve_id));
                return None;
            }
        };
        if baked.points.iter().flatten().any(|v| !v.is_finite()) {
            self.report(
                file_of(curve_id),
                "the curve bakes a breakpoint that is not a finite number",
            );
            return None;
        }
        (!baked.points.is_empty()).then_some(baked)
    }

    /// The ERRORs the bake gathered, and the weight it spent.
    pub fn finish(self) -> (Vec<Finding>, usize) {
        (self.findings, self.budget.spent())
    }
}
