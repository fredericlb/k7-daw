// The mastering surface's vocabulary: three parts, the required ten, the optional three, and the gate over a mapping.

use super::*;
use crate::bindings::unmapped_controls;
use serde_json::{Value, json};

fn mapping(v: Value) -> IndexMap<String, Option<SurfaceBinding>> {
    serde_json::from_value(v).unwrap_or_default()
}

fn required() -> Vec<String> {
    MASTER_CONTROLS
        .iter()
        .filter(|c| !c.optional)
        .map(|c| c.id.to_owned())
        .collect()
}

fn wired(module: &str) -> Value {
    MASTER_CONTROLS
        .iter()
        .map(|c| (c.id.to_owned(), json!({"module": module, "param": "x"})))
        .collect::<serde_json::Map<_, _>>()
        .into()
}

#[test]
fn every_control_is_in_one_of_the_three_parts_and_every_part_has_one() {
    // The parts ARE the node: a control in none of them would resolve, move a param, and be drawn nowhere.
    for c in &MASTER_CONTROLS {
        assert!(MASTER_PARTS.contains(&c.part), "{} is in no part", c.id);
        assert!(
            c.id.starts_with(&format!("{}.", c.part)),
            "{} is not prefixed by its part",
            c.id
        );
    }
    for part in MASTER_PARTS {
        assert!(MASTER_CONTROLS.iter().any(|c| c.part == part));
    }
}

#[test]
fn every_home_is_inside_its_own_range() {
    // `home` is what a knob draws before the engine has said anything; outside the range it would draw at an end, then jump.
    for c in &MASTER_CONTROLS {
        assert!(c.range.0 < c.range.1, "{}", c.id);
        assert!(c.home >= c.range.0 && c.home <= c.range.1, "{}", c.id);
    }
}

#[test]
fn the_ten_are_required_and_three_stages_are_offered() {
    assert_eq!(required().len(), 10);
    let optional: Vec<&str> = MASTER_CONTROLS.iter().filter(|c| c.optional).map(|c| c.id).collect();
    assert_eq!(optional, ["comp.lowCut", "limiter.hfLimit", "limiter.output"]);
    // There is no smaller mastering strip: a missing line is the same strip with a dead knob, asserted through the gate.
    assert_eq!(unmapped_controls(&required(), &IndexMap::new()), required());
}

#[test]
fn an_absent_line_and_a_null_one_are_the_same_gap() {
    // A scaffolded file is made of `null`s, so a gate that only looked for absence would draw ten dead knobs.
    let full = wired("qm");
    assert!(unmapped_controls(&required(), &mapping(full.clone())).is_empty());
    let mut nulled = full.clone();
    nulled["comp.low"] = Value::Null;
    assert_eq!(unmapped_controls(&required(), &mapping(nulled)), ["comp.low"]);
    let mut short = full.clone();
    short.as_object_mut().map(|m| m.remove("spread.width"));
    assert_eq!(unmapped_controls(&required(), &mapping(short)), ["spread.width"]);
    // A surface written before the three optional stages existed still draws: their lines are OFFERED, not required.
    let mut old = full;
    for id in ["comp.lowCut", "limiter.hfLimit", "limiter.output"] {
        old.as_object_mut().map(|m| m.remove(id));
    }
    assert!(unmapped_controls(&required(), &mapping(old)).is_empty());
}

#[test]
fn one_module_is_read_out_of_ten_lines_naming_it() {
    // The shape the strip is drawn for, and why `mapped_modules` deduplicates: more than one is legal and worth SAYING.
    let one = mapping(wired("master_qm_1"));
    assert_eq!(mapped_modules(&one), ["master_qm_1"]);
    let mut two = wired("master_qm_1");
    two["limiter.drive"] = json!({"module": "other", "param": "x"});
    assert_eq!(mapped_modules(&mapping(two)), ["master_qm_1", "other"]);
    assert!(mapped_modules(&IndexMap::new()).is_empty());
}
