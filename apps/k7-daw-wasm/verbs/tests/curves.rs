// curves.rs — curve files and their bindings on the `curves` fixture: new, set, points, and what the engine then bakes.
// A reader would get wrong where a binding lives: on the MODULE's node file (nodes/lead.k7x here), never on the lane.

mod support;

use k7x::edit::Value;
use support::Project;

/// The baked automation of `param` on lane `melody`, as the engine is sent it.
fn baked(p: &mut Project, param: &str) -> Option<Vec<[f64; 2]>> {
    p.track("melody")
        .automation
        .into_iter()
        .find(|a| a.param == param)
        .map(|a| a.points)
}

/// The values of a drawn curve's points, in file order.
fn values(p: &Project, id: &str) -> Vec<f64> {
    match p.data(&format!("curves/{id}.k7x")).get("points") {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|pt| match pt.get("value") {
                Some(Value::Num(n)) => Some(*n),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The params `nodes/lead.k7x` binds, each with its curve.
fn bindings(p: &Project) -> Vec<(String, String)> {
    match p.data("nodes/lead.k7x").get("automations") {
        Some(Value::List(l)) => l
            .iter()
            .map(|b| {
                let s = |k: &str| match b.get(k) {
                    Some(Value::Str(s)) => s.clone(),
                    _ => String::new(),
                };
                (s("param"), s("curve"))
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[test]
fn a_new_curve_is_a_sine_lfo_and_never_overwrites() {
    let mut p = Project::open("curves");
    let ran = p.round_trip("curve new rise");
    assert_eq!(ran.answer.log, ["created curves/rise.k7x"]);
    assert_eq!(ran.changed(&p.dir), ["curves/rise.k7x"]);
    p.ok("curve new rise");
    assert!(p.text("curves/rise.k7x").starts_with("type(\"lfo\")\nshape(\"sine\")"));
    p.refused("curve new sweep", "curve 'sweep' already exists (curves/sweep.k7x)");
    p.refused("curve new a.b", "invalid curve name 'a.b'");
    p.refused("curve new", "usage: curve new <name>");
}

#[test]
fn a_set_writes_the_whole_curve_in_its_kinds_order() {
    let mut p = Project::open("curves");
    let line = r#"curve set wobble {"frequency":2,"type":"lfo","polarity":"bipolar","shape":"triangle"}"#;
    let before = baked(&mut p, "Osc1 Tune").expect("wobble is baked");
    let ran = p.round_trip(line);
    assert_eq!(ran.answer.log, ["wrote curves/wobble.k7x"]);
    p.ok(line);
    assert_eq!(
        p.text("curves/wobble.k7x"),
        "type(\"lfo\")\nshape(\"triangle\")\nfrequency(2)\npolarity(\"bipolar\")\n"
    );
    assert_ne!(
        baked(&mut p, "Osc1 Tune").expect("still baked"),
        before,
        "the engine plays the new shape"
    );
    let made =
        p.ok(r#"curve set ramp {"type":"custom","points":[{"time":"1:1:1","value":0},{"time":"2:1:1","value":1}]}"#);
    assert_eq!(made.answer.log, ["created curves/ramp.k7x"]);
    assert_eq!(values(&p, "ramp"), [0.0, 1.0]);
    p.refused(
        r#"curve set wobble {"type":"lfo","frequency":-1}"#,
        "curve set: curve 'wobble'",
    );
    p.refused(
        r#"curve set wobble {"type":"lfo","speed":3}"#,
        "curve set: curve 'wobble'",
    );
    p.refused(r#"curve set wobble {"type":"spline"}"#, "curve set: curve 'wobble'");
    p.refused("curve set wobble [1]", "usage: curve set <name> <json>");
    p.refused(r#"curve set bad.name {"type":"lfo"}"#, "invalid curve name");
}

#[test]
fn points_are_added_moved_and_removed_by_time() {
    let mut p = Project::open("curves");
    let before = baked(&mut p, "Filter Freq").expect("sweep is baked");
    let ran = p.round_trip("curve point add sweep 4:1:1 0.5");
    assert_eq!(
        ran.answer.log,
        ["sweep: added a point at 4:1:1 to 0.5 (curves/sweep.k7x)"]
    );
    p.ok("curve point add sweep 4:1:1 0.5");
    assert_eq!(values(&p, "sweep"), [0.0, 1.0, 0.2, 0.5]);
    assert_ne!(baked(&mut p, "Filter Freq").expect("baked"), before);
    // A point between two goes between them; one at a taken time sets it; a value past 1 is 1.
    p.ok("curve point add sweep 1:3:1 7");
    assert_eq!(values(&p, "sweep"), [0.0, 1.0, 1.0, 0.2, 0.5]);
    let set = p.ok("curve point add sweep 1:1:1 0.25");
    assert_eq!(
        set.answer.log[0],
        "sweep: set the point at 1:1:1 to 0.25 (curves/sweep.k7x)"
    );
    p.round_trip("curve point move sweep 2:1:1 5:1:1");
    p.ok("curve point move sweep 2:1:1 5:1:1 0.75");
    assert_eq!(values(&p, "sweep"), [0.25, 1.0, 0.2, 0.5, 0.75]);
    p.round_trip("curve point remove sweep 3:1:1");
    p.ok("curve point remove sweep 3:1:1");
    assert_eq!(values(&p, "sweep"), [0.25, 1.0, 0.5, 0.75]);
    p.refused(
        "curve point move sweep 9:1:1 10:1:1",
        "nothing to edit in curve 'sweep'",
    );
    p.refused("curve point add wobble 2:1:1 0.5", "not a drawn curve");
    p.refused("curve point add nope 2:1:1 0.5", "curve point add:");
    p.refused("curve point add sweep 2:1:1", "usage: curve point add");
}

#[test]
fn the_last_point_stays() {
    let mut p = Project::open("curves");
    p.ok(r#"curve set one {"type":"custom","points":[{"time":"1:1:1","value":0.5}]}"#);
    p.refused("curve point remove one 1:1:1", "has one point left");
}

#[test]
fn node_automate_binds_on_the_node_and_none_unbinds() {
    let mut p = Project::open("curves");
    let bound = p.round_trip("node automate lead Level 2 sweep");
    assert_eq!(bound.answer.log, ["lead: Level 2 <- sweep"]);
    assert_eq!(bound.changed(&p.dir), ["nodes/lead.k7x"]);
    p.ok("node automate lead Level 2 sweep");
    assert_eq!(bindings(&p), [("Level 2".to_owned(), "@meta/curves/sweep".to_owned())]);
    assert!(baked(&mut p, "Level 2").is_some(), "the engine is sent Level 2's curve");
    // The node's own list stands for the module's whole: the inherited bindings are no longer heard.
    assert!(baked(&mut p, "Filter Freq").is_none());
    let ran = p.round_trip("node automate lead Level 2 none");
    assert_eq!(
        ran.answer.log,
        [format!(
            "lead: Level 2 is no longer automated ({}/nodes/lead.k7x)",
            p.dir
        )]
    );
    p.ok("node automate lead Level 2 none");
    assert!(bindings(&p).is_empty());
    assert!(baked(&mut p, "Level 2").is_none());
    assert!(baked(&mut p, "Filter Freq").is_some(), "the module's bindings are back");
    // A binding the node inherits from its module is not the node's to drop: `none` writes nothing.
    let inherited = p.ok("node automate lead Filter Freq none");
    assert_eq!(inherited.before, inherited.after);
    assert!(baked(&mut p, "Filter Freq").is_some());
    p.refused("node automate lead Filter Type sweep", "cannot be automated");
    p.refused("node automate lead sweep", "usage: node automate");
}

#[test]
fn track_automation_is_a_view_from_the_lane() {
    let mut p = Project::open("curves");
    let ran = p.round_trip("track automation add melody Filter Resonance");
    let said = &ran.answer.log[0];
    assert!(
        said.starts_with(
            "melody: Filter Resonance <- melody-Filter-Resonance (curves/melody-Filter-Resonance.k7x), flat at "
        ) && said.ends_with("/nodes/lead.k7x)"),
        "{said}"
    );
    assert_eq!(
        ran.changed(&p.dir),
        ["curves/melody-Filter-Resonance.k7x", "nodes/lead.k7x"]
    );
    p.ok("track automation add melody Filter Resonance");
    let flat_values = values(&p, "melody-Filter-Resonance");
    assert!(
        flat_values.len() == 2 && flat_values[0] == flat_values[1],
        "{flat_values:?}"
    );
    let flat = baked(&mut p, "Filter Resonance").expect("the flat curve is baked");
    assert!(flat.iter().all(|pt| (pt[1] - flat[0][1]).abs() < 1e-9), "{flat:?}");
    p.refused("track automation add melody Filter Resonance", "already exists");
    p.round_trip("track automation remove melody Filter Resonance");
    p.ok("track automation remove melody Filter Resonance");
    assert!(p.exists("curves/melody-Filter-Resonance.k7x"), "the curve file stays");
    assert!(baked(&mut p, "Filter Resonance").is_none());
    p.refused(
        "track automation add melody Cutoff",
        "no parameter 'Cutoff' on 'melody'",
    );
    p.refused("track automation add nobody Level", "no track \"nobody\"");
    p.refused("track automation add melody", "usage: track automation add");
}
