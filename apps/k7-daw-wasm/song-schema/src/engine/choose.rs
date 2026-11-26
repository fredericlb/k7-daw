// choose.rs — which engine to drive, given what the account remembers: the account's choice first, an obvious answer second, a question only when there is one.
// A reader would get wrong that anyone may register an engine: only a trusted one (the remembered, or one reached directly) is taken unasked.

use super::quality::Quality;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// What the policy reads of one probed engine; the page works out the address facts, which need a URL parse.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub id: String,
    /// It answered with an address and a token: unreachable engines are never offered.
    pub reachable: bool,
    /// It answered on a direct socket to one of its own private addresses.
    pub direct: bool,
    /// It answered over loopback: the engine whose speakers you sit in front of.
    pub here: bool,
    /// Another person's session holds it.
    pub assigned: bool,
    /// What its link measured; none when nothing answered.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub latency: Option<Latency>,
    /// The machine's rating, when it reports one.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub performance: Option<f64>,
    /// Its compatibility with the project, 0 to 100; none when it could not be told.
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub compat: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct Latency {
    pub quality: Quality,
    pub total_ms: f64,
}

/// The engine to use, as an index into the candidates, or none when the user has to be asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Choice {
    pub index: Option<usize>,
    /// Several were reachable and none remembered; a single reachable engine is the answer, not a choice.
    pub ambiguous: bool,
}

/// Whether this engine may be taken with nobody clicking: the account's own pick, or one reached directly.
fn may_auto_pick(c: &Candidate, remembered: Option<&str>) -> bool {
    remembered == Some(c.id.as_str()) || (c.reachable && c.direct)
}

/// Picks from what the registry offered; a remembered engine being away does not clear the preference.
pub fn choose_engine(candidates: &[Candidate], remembered: Option<&str>) -> Choice {
    let reachable: Vec<usize> = (0..candidates.len()).filter(|&i| candidates[i].reachable).collect();
    let remembered = remembered.filter(|r| !r.is_empty());
    if let Some(r) = remembered
        && let Some(&i) = reachable.iter().find(|&&i| candidates[i].id == r)
    {
        return Choice {
            index: Some(i),
            ambiguous: false,
        };
    }
    if let [only] = reachable[..]
        && may_auto_pick(&candidates[only], remembered)
    {
        return Choice {
            index: Some(only),
            ambiguous: false,
        };
    }
    Choice {
        index: None,
        ambiguous: reachable.len() > 1,
    }
}

fn desc(a: f64, b: f64) -> Ordering {
    b.partial_cmp(&a).unwrap_or(Ordering::Equal)
}

/// The engine to take when nobody has to be asked: compatibility, then the remembered, this machine, the latency class, the machine's rating, the latency.
pub fn best_engine(candidates: &[Candidate], remembered: Option<&str>) -> Option<usize> {
    let remembered = remembered.filter(|r| !r.is_empty());
    let mut ranked: Vec<(usize, &Candidate, Latency)> = candidates
        .iter()
        .enumerate()
        .filter_map(|(i, c)| match c.latency {
            Some(l) if c.reachable && !c.assigned && may_auto_pick(c, remembered) => Some((i, c, l)),
            _ => None,
        })
        .collect();
    let flag = |b: bool| if b { 1.0 } else { 0.0 };
    let is_remembered = |c: &Candidate| flag(remembered == Some(c.id.as_str()));
    ranked.sort_by(|(_, a, la), (_, b, lb)| {
        desc(a.compat.unwrap_or(-1.0), b.compat.unwrap_or(-1.0))
            .then_with(|| desc(is_remembered(a), is_remembered(b)))
            .then_with(|| desc(flag(a.here), flag(b.here)))
            .then_with(|| la.quality.cmp_band(lb.quality))
            .then_with(|| desc(a.performance.unwrap_or(0.0), b.performance.unwrap_or(0.0)))
            .then_with(|| la.total_ms.partial_cmp(&lb.total_ms).unwrap_or(Ordering::Equal))
    });
    ranked.first().map(|(i, _, _)| *i)
}

impl Quality {
    /// Q1 before Q2 before Q3.
    fn cmp_band(self, other: Quality) -> Ordering {
        (self as u8).cmp(&(other as u8))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(id: &str, ms: Option<f64>) -> Candidate {
        Candidate {
            id: id.into(),
            reachable: ms.is_some(),
            direct: true,
            here: false,
            assigned: false,
            latency: ms.map(|total_ms| Latency {
                quality: Quality::Q1,
                total_ms,
            }),
            performance: None,
            compat: None,
        }
    }

    fn rated(id: &str, ms: Option<f64>, quality: Quality, score: Option<f64>, assigned: bool) -> Candidate {
        Candidate {
            latency: ms.map(|total_ms| Latency { quality, total_ms }),
            performance: score,
            assigned,
            ..engine(id, ms)
        }
    }

    fn q1(id: &str, ms: f64, score: f64) -> Candidate {
        rated(id, Some(ms), Quality::Q1, Some(score), false)
    }

    fn id_of(list: &[Candidate], i: Option<usize>) -> Option<&str> {
        i.map(|i| list[i].id.as_str())
    }

    fn compat(c: Candidate, score: f64) -> Candidate {
        Candidate {
            compat: Some(score),
            ..c
        }
    }

    fn relayed(c: Candidate) -> Candidate {
        Candidate { direct: false, ..c }
    }

    #[test]
    fn takes_the_remembered_one_even_when_others_are_reachable() {
        let l = [engine("a", Some(5.0)), engine("b", Some(30.0))];
        let c = choose_engine(&l, Some("b"));
        assert_eq!((id_of(&l, c.index), c.ambiguous), (Some("b"), false));
    }

    #[test]
    fn takes_the_only_reachable_one_with_nothing_remembered() {
        let l = [engine("a", Some(5.0))];
        assert_eq!(choose_engine(&l, None).index, Some(0));
    }

    #[test]
    fn asks_only_when_several_are_reachable_and_none_is_remembered() {
        let l = [engine("a", Some(5.0)), engine("b", Some(30.0))];
        assert_eq!(
            choose_engine(&l, None),
            Choice {
                index: None,
                ambiguous: true
            }
        );
    }

    #[test]
    fn never_offers_an_unreachable_engine_remembered_or_not() {
        let l = [engine("far", None)];
        assert_eq!(choose_engine(&l, Some("far")).index, None);
        assert!(!choose_engine(&l, None).ambiguous);
    }

    #[test]
    fn falls_back_to_the_picker_rather_than_to_another_machine() {
        let l = [engine("a", Some(5.0)), engine("b", Some(30.0))];
        assert_eq!(
            choose_engine(&l, Some("gone")),
            Choice {
                index: None,
                ambiguous: true
            }
        );
    }

    #[test]
    fn has_nothing_to_offer_when_the_registry_is_empty() {
        assert_eq!(
            choose_engine(&[], Some("a")),
            Choice {
                index: None,
                ambiguous: false
            }
        );
    }

    #[test]
    fn prefers_the_better_latency_tier_over_a_stronger_machine() {
        let l = [
            rated("far", Some(40.0), Quality::Q2, Some(3000.0), false),
            q1("near", 8.0, 900.0),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("near"));
    }

    #[test]
    fn within_a_tier_takes_the_stronger_machine_over_the_lower_latency() {
        let l = [q1("fast-link", 4.0, 800.0), q1("fast-box", 9.0, 1600.0)];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("fast-box"));
    }

    #[test]
    fn falls_back_to_latency_when_neither_reports_a_score() {
        let l = [
            rated("b", Some(9.0), Quality::Q1, None, false),
            rated("a", Some(4.0), Quality::Q1, None, false),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("a"));
    }

    #[test]
    fn never_takes_an_unreachable_or_in_use_engine() {
        let l = [
            rated("gone", None, Quality::Q1, Some(5000.0), false),
            rated("busy", Some(2.0), Quality::Q1, Some(5000.0), true),
            rated("ok", Some(30.0), Quality::Q2, Some(500.0), false),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("ok"));
        assert_eq!(
            best_engine(&[rated("busy", Some(2.0), Quality::Q1, Some(900.0), true)], None),
            None
        );
        assert_eq!(best_engine(&[], None), None);
    }

    #[test]
    fn compares_compatibility_before_the_link_and_the_machine() {
        let l = [
            compat(q1("fast", 2.0, 3000.0), 50.0),
            compat(rated("slow", Some(40.0), Quality::Q3, Some(500.0), false), 100.0),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("slow"));
    }

    #[test]
    fn among_equally_compatible_engines_falls_back_to_link_then_machine() {
        let l = [
            compat(q1("partial", 2.0, 3000.0), 80.0),
            compat(q1("full-weak", 4.0, 800.0), 100.0),
            compat(q1("full-strong", 9.0, 1600.0), 100.0),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("full-strong"));
    }

    #[test]
    fn ranks_an_engine_whose_compatibility_is_unknown_below_any_known_score() {
        let l = [
            q1("silent", 2.0, 3000.0),
            compat(rated("known", Some(30.0), Quality::Q2, Some(500.0), false), 0.0),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("known"));
    }

    #[test]
    fn orders_by_compatibility_then_latency_class_then_performance() {
        let all = vec![
            compat(q1("partial", 1.0, 5000.0), 90.0),
            compat(
                rated("full-q2-strong", Some(20.0), Quality::Q2, Some(4000.0), false),
                100.0,
            ),
            compat(q1("full-q1-weak", 3.0, 700.0), 100.0),
            compat(q1("full-q1-strong", 8.0, 1500.0), 100.0),
        ];
        assert_eq!(id_of(&all, best_engine(&all, None)), Some("full-q1-strong"));
        let without: Vec<_> = all.into_iter().filter(|c| c.id != "full-q1-strong").collect();
        assert_eq!(id_of(&without, best_engine(&without, None)), Some("full-q1-weak"));
    }

    #[test]
    fn prefers_the_engine_on_this_machine_over_a_stronger_one_elsewhere() {
        let here = Candidate {
            here: true,
            ..q1("mac", 12.0, 7000.0)
        };
        let l = [q1("studio-pc", 8.0, 12000.0), here];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("mac"));
    }

    #[test]
    fn takes_the_engine_this_account_last_chose_over_this_machines() {
        let here = Candidate {
            here: true,
            ..q1("mac", 2.0, 7000.0)
        };
        let l = [here, rated("studio-pc", Some(20.0), Quality::Q2, Some(3000.0), false)];
        assert_eq!(id_of(&l, best_engine(&l, Some("studio-pc"))), Some("studio-pc"));
    }

    #[test]
    fn falls_back_on_this_machine_when_the_remembered_one_is_unreachable() {
        let here = Candidate {
            here: true,
            ..q1("mac", 12.0, 7000.0)
        };
        let l = [rated("studio-pc", None, Quality::Q1, Some(12000.0), false), here];
        assert_eq!(id_of(&l, best_engine(&l, Some("studio-pc"))), Some("mac"));
    }

    #[test]
    fn still_puts_compatibility_before_this_machine() {
        let here = Candidate {
            here: true,
            ..compat(q1("mac", 2.0, 7000.0), 60.0)
        };
        let l = [here, compat(q1("studio-pc", 8.0, 3000.0), 100.0)];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("studio-pc"));
    }

    #[test]
    fn never_auto_picks_an_engine_reached_only_through_the_relay() {
        assert_eq!(best_engine(&[relayed(q1("evil", 2.0, 99999.0))], None), None);
        assert_eq!(choose_engine(&[relayed(engine("evil", Some(2.0)))], None).index, None);
    }

    #[test]
    fn prefers_a_slower_direct_engine_over_a_faster_relayed_stranger() {
        let l = [
            relayed(q1("evil", 1.0, 99999.0)),
            rated("lan", Some(30.0), Quality::Q2, Some(500.0), false),
        ];
        assert_eq!(id_of(&l, best_engine(&l, None)), Some("lan"));
    }

    #[test]
    fn takes_a_relayed_engine_the_account_remembered() {
        let l = [relayed(rated("mine", Some(9.0), Quality::Q1, None, false))];
        assert_eq!(best_engine(&l, Some("mine")), Some(0));
        assert_eq!(
            choose_engine(&[relayed(engine("mine", Some(9.0)))], Some("mine")).index,
            Some(0)
        );
    }
}
