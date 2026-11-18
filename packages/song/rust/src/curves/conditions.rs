// curves/conditions.rs — WHEN a curve runs: the stretches of song time it is sampled over, and an envelope's firings.
// A reader would get wrong that an event condition walks the trigger list: only a note-on event does, and the walk is weighed.

use super::resolved::{Condition, ResolvedCurve};
use super::{Budget, Over};

/// One stretch a curve runs over, in beats: `[from, to]`, its own time counted from `origin`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveRun {
    pub from: f64,
    pub to: f64,
    pub origin: f64,
}

/// One firing of an envelope: when, where its condition's window cuts it, and whether it may cut one running.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnvelopeFire {
    pub at: f64,
    pub stop: Option<f64>,
    pub retrigger: bool,
}

/// The one event a node announces today: a note-on, however it is spelt ("Note On", "noteOn", "note-on").
pub fn is_note_on_event(name: &str) -> bool {
    let letters: String = name.to_lowercase().chars().filter(|c| c.is_ascii_lowercase()).collect();
    letters == "noteon"
}

fn by_beat(a: f64, b: f64) -> std::cmp::Ordering {
    (a + 0.0).total_cmp(&(b + 0.0))
}

/// The triggers sorted, once, however many conditions read them.
fn sorted_triggers(triggers: &[f64]) -> Vec<f64> {
    let mut ts = triggers.to_vec();
    ts.sort_by(|a, b| by_beat(*a, *b));
    ts
}

/// The stretches a song-time curve runs over, any condition sufficing; none is the whole song.
pub fn curve_runs(c: &ResolvedCurve, end: f64, triggers: &[f64], budget: &mut Budget) -> Result<Vec<CurveRun>, Over> {
    if c.conditions.is_empty() {
        budget.spend(1)?;
        return Ok(vec![CurveRun {
            from: 0.0,
            to: end,
            origin: 0.0,
        }]);
    }
    let mut all: Vec<CurveRun> = Vec::new();
    let mut sorted: Option<Vec<f64>> = None;
    for cond in &c.conditions {
        match *cond {
            Condition::Start { at, retrigger } if at < end => all.push(CurveRun {
                from: at,
                to: end,
                origin: if retrigger { at } else { 0.0 },
            }),
            Condition::Start { .. } => {}
            Condition::End { at, .. } => all.push(CurveRun {
                from: 0.0,
                to: at,
                origin: 0.0,
            }),
            Condition::Between {
                start,
                end: stop,
                retrigger,
            } if stop > start => all.push(CurveRun {
                from: start,
                to: stop,
                origin: if retrigger { start } else { 0.0 },
            }),
            Condition::Between { .. } => {}
            Condition::Event { note_on, retrigger } => {
                if !note_on || triggers.is_empty() {
                    continue;
                }
                // The list is paid when it is sorted, once, and again for each run a retrigger lays from it.
                if sorted.is_none() {
                    budget.spend(triggers.len())?;
                }
                let ts = sorted.get_or_insert_with(|| sorted_triggers(triggers));
                budget.spend(if retrigger { ts.len() } else { 1 })?;
                if retrigger {
                    for (i, t) in ts.iter().enumerate() {
                        let to = match ts.get(i + 1) {
                            Some(next) => *next,
                            None => end.max(*t),
                        };
                        all.push(CurveRun {
                            from: *t,
                            to,
                            origin: *t,
                        });
                    }
                } else if let Some(first) = ts.first() {
                    all.push(CurveRun {
                        from: *first,
                        to: end.max(*first),
                        origin: *first,
                    });
                }
            }
        }
        budget.spend(1)?;
    }
    budget.spend(all.len())?;
    all.sort_by(|a, b| by_beat(a.from, b.from));
    let mut out: Vec<CurveRun> = Vec::new();
    for r in all {
        let Some(last) = out.last_mut() else {
            out.push(r);
            continue;
        };
        if r.from > last.to {
            out.push(r);
        } else if r.origin == r.from && r.from > last.from && r.origin != last.origin {
            // A run that RESTARTS the curve takes over where it starts; the one it cut keeps nothing after it.
            let to = r.to.max(last.to);
            last.to = r.from;
            out.push(CurveRun { to, ..r });
        } else {
            // One that carries on simply widens the stretch already running.
            last.to = last.to.max(r.to);
        }
    }
    Ok(out)
}

/// One condition's firings.
fn fires_of(cond: &Condition, note_ons: &[f64], out: &mut Vec<EnvelopeFire>) {
    let (from, stop, retrigger) = match *cond {
        Condition::Event { note_on, retrigger } => {
            if note_on {
                out.extend(note_ons.iter().map(|t| EnvelopeFire {
                    at: *t,
                    stop: None,
                    retrigger,
                }));
            }
            return;
        }
        Condition::Start { at, retrigger } => (at, None, retrigger),
        Condition::End { at, retrigger } => (0.0, Some(at), retrigger),
        Condition::Between { start, end, retrigger } => (start, Some(end), retrigger),
    };
    let inside = |t: &&f64| **t >= from && stop.is_none_or(|s| **t < s);
    // Retriggered, it also fires the moment the condition comes true: the window's opening.
    if retrigger && from > 0.0 {
        out.push(EnvelopeFire {
            at: from,
            stop,
            retrigger,
        });
        out.extend(
            note_ons
                .iter()
                .filter(inside)
                .filter(|t| **t > from)
                .map(|t| EnvelopeFire {
                    at: *t,
                    stop,
                    retrigger,
                }),
        );
    } else {
        out.extend(note_ons.iter().filter(inside).map(|t| EnvelopeFire {
            at: *t,
            stop,
            retrigger,
        }));
    }
}

/// An envelope's firings under its conditions, pooled: any condition's note-ons fire it, each cut where ITS window ends.
pub fn envelope_fires(c: &ResolvedCurve, note_ons: &[f64], budget: &mut Budget) -> Result<Vec<EnvelopeFire>, Over> {
    // No condition is what an envelope always did: every note-on fires it, cutting the one running.
    if c.conditions.is_empty() {
        budget.spend(note_ons.len())?;
        return Ok(note_ons
            .iter()
            .map(|at| EnvelopeFire {
                at: *at,
                stop: None,
                retrigger: true,
            })
            .collect());
    }
    let mut fires: Vec<EnvelopeFire> = Vec::new();
    for cond in &c.conditions {
        // A condition that walks the note-ons pays for it, and may lay one more for a window's opening.
        if matches!(cond, Condition::Event { note_on: false, .. }) {
            continue;
        }
        budget.spend(note_ons.len().saturating_add(1))?;
        fires_of(cond, note_ons, &mut fires);
    }
    fires.sort_by(|a, b| by_beat(a.at, b.at));
    // Two conditions firing at one beat are one firing: the longer window, and retriggering if either does.
    let mut out: Vec<EnvelopeFire> = Vec::new();
    for f in fires {
        match out.last_mut() {
            Some(was) if was.at == f.at => {
                was.retrigger = was.retrigger || f.retrigger;
                was.stop = match (was.stop, f.stop) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
            }
            _ => out.push(f),
        }
    }
    Ok(out)
}
