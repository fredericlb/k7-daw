// freeze_graph.rs — the freeze graph: what is upstream of a module, when it is refused, and the plan (freeze.test.ts, decisions 336 to 338).
// A reader would get wrong that a Blocked is data: the TypeScript's sentences are the view's, so their wording is not held here.

use crate::freeze_support;

use freeze_support::{chain, chains, fast, sorted};
use indexmap::IndexMap;
use song::clips::Budget;
use song::freeze::{BUDGET, Blocked, Edges, freeze_plans};
use song::routing::{Chain, UseStep};

fn budget() -> Budget {
    Budget::new(BUDGET)
}

fn run() -> IndexMap<String, Chain> {
    chains(vec![("gtr", chain(Some("gtr"), &["amp", "cab"], &["OUT"]))])
}

fn bus() -> IndexMap<String, Chain> {
    chains(vec![
        ("gtr", chain(Some("gtr"), &[], &["verb"])),
        ("keys", chain(Some("keys"), &[], &["verb"])),
        ("verb", chain(None, &["verb", "plate"], &["OUT"])),
    ])
}

fn up(c: &IndexMap<String, Chain>, module: &str) -> Vec<String> {
    let e = Edges::of(c);
    let set = e.upstream_of(module, &mut budget()).expect("within the budget");
    sorted(set.iter().copied())
}

#[test]
fn upstream_includes_the_module_itself() {
    // freeze.test.ts "includes the module ITSELF"; decision 336
    assert_eq!(up(&run(), "amp"), ["amp", "gtr"]);
}

#[test]
fn upstream_stops_there_and_downstream_stays_live() {
    // freeze.test.ts "stops there"; decision 336
    assert!(!up(&run(), "amp").contains(&"cab".to_owned()));
    assert_eq!(up(&run(), "cab"), ["amp", "cab", "gtr"]);
}

#[test]
fn upstream_takes_every_lane_that_meets_at_a_bus() {
    // freeze.test.ts "takes EVERY lane that meets at a bus"; decision 336
    assert_eq!(up(&bus(), "verb"), ["gtr", "keys", "verb"]);
    assert_eq!(up(&bus(), "plate"), ["gtr", "keys", "plate", "verb"]);
}

#[test]
fn upstream_follows_a_side_input() {
    // freeze.test.ts "follows a SIDE input"; decision 336
    let mut a = chain(Some("deckA"), &["crossfade"], &["OUT"]);
    a.use_steps[0] = UseStep {
        module: "crossfade".into(),
        inputs: Some(vec!["deckB".into()]),
        enabled: None,
    };
    let c = chains(vec![
        ("deckA", a),
        ("deckB", chain(Some("deckB"), &[], &["crossfade#b"])),
    ]);
    assert_eq!(up(&c, "crossfade"), ["crossfade", "deckA", "deckB"]);
}

#[test]
fn upstream_stops_at_a_cycle() {
    // freeze.test.ts "stops at a cycle instead of looping on it"; decision 336
    let c = chains(vec![("a", chain(None, &["a", "b"], &["a"]))]);
    assert!(up(&c, "b").contains(&"a".to_owned()));
}

#[test]
fn upstream_takes_a_lanes_own_run_when_the_point_is_the_lanes_last_module() {
    // freeze.test.ts "takes a lane's own run"; decision 336
    let c = chains(vec![
        ("gtr", chain(Some("gtr"), &["amp"], &["verb"])),
        ("verb", chain(None, &["verb"], &["OUT"])),
    ]);
    assert_eq!(up(&c, "verb"), ["amp", "gtr", "verb"]);
}

#[test]
fn a_name_in_no_edge_has_no_upstream_here() {
    // decision 336: the TypeScript answers the name alone, which cannot be borrowed from the chains
    assert!(up(&run(), "ghost").is_empty());
}

fn blocked(c: &IndexMap<String, Chain>, module: &str) -> Option<Blocked> {
    let e = Edges::of(c);
    let inside = e.upstream_of(module, &mut budget()).expect("budget");
    e.blocked_by(module, &inside, &mut budget()).expect("budget")
}

fn dry_gtr() -> IndexMap<String, Chain> {
    chains(vec![
        ("gtr", chain(Some("gtr"), &[], &["amp", "OUT"])),
        ("amp", chain(None, &["amp"], &["OUT"])),
    ])
}

#[test]
fn it_refuses_when_something_upstream_also_feeds_outside_the_set() {
    // freeze.test.ts "refuses when something upstream ALSO feeds outside"; decision 337
    assert_eq!(
        blocked(&dry_gtr(), "amp"),
        Some(Blocked::FeedsOutside {
            from: "gtr".into(),
            to: "OUT".into()
        })
    );
}

#[test]
fn the_message_of_the_way_out_is_not_held_here() {
    // freeze.test.ts "names the way out" pins a sentence; decision 337 leaves the wording to the view, so only the data is held
    assert!(matches!(blocked(&dry_gtr(), "amp"), Some(Blocked::FeedsOutside { .. })));
}

#[test]
fn the_modules_own_destinations_may_leave() {
    // freeze.test.ts "allows the module's OWN destinations to leave"; decision 337
    assert_eq!(blocked(&run(), "amp"), None);
    assert_eq!(blocked(&bus(), "verb"), None);
}

#[test]
fn a_split_that_rejoins_is_allowed() {
    // freeze.test.ts "allows a lane that feeds two things BOTH inside the set"; decision 337
    let c = chains(vec![
        ("gtr", chain(Some("gtr"), &[], &["dry", "wet"])),
        ("dry", chain(None, &["dry"], &["mix"])),
        ("wet", chain(None, &["wet"], &["mix"])),
        ("mix", chain(None, &["mix"], &["OUT"])),
    ]);
    assert_eq!(blocked(&c, "mix"), None);
    assert_eq!(up(&c, "mix"), ["dry", "gtr", "mix", "wet"]);
}

#[test]
fn a_name_in_no_chain_is_no_such_module() {
    // freeze.test.ts "says so when the name is in no chain at all"; decision 337
    assert_eq!(blocked(&run(), "ghost"), Some(Blocked::NoSuchModule));
}

#[test]
fn a_destination_alone_is_a_name_but_no_node() {
    // freeze.ts hasNode: only a lane or a step is a node, and OUT is neither
    let c = run();
    let e = Edges::of(&c);
    assert!(e.has_node("gtr") && e.has_node("amp") && !e.has_node("OUT"));
    assert!(e.on_a_chain("amp") && !e.on_a_chain("gtr"));
}

fn is_lane(n: &str) -> bool {
    n == "gtr" || n == "keys"
}

#[test]
fn the_plan_splits_the_set_into_lanes_and_modules() {
    // freeze.test.ts "splits the set into lanes ... and modules"; decision 338
    let c = bus();
    let e = Edges::of(&c);
    let at = [("verb".to_owned(), "renders/verb.wav".to_owned())];
    let plans = freeze_plans(&e, &at, &is_lane, &mut budget()).expect("budget");
    let plan = &plans[0];
    assert_eq!((plan.at.as_str(), plan.render.as_str()), ("verb", "renders/verb.wav"));
    assert_eq!(sorted(plan.lanes.iter().map(String::as_str)), ["gtr", "keys"]);
    assert_eq!(plan.modules.iter().collect::<Vec<_>>(), ["verb"]);
}

#[test]
fn a_frozen_name_the_chains_do_not_have_is_dropped() {
    // freeze.test.ts "drops a frozen name the chains do not have"; decision 338
    let c = run();
    let e = Edges::of(&c);
    let at = [
        ("ghost".to_owned(), "renders/ghost.wav".to_owned()),
        ("amp".to_owned(), "renders/amp.wav".to_owned()),
    ];
    let plans = freeze_plans(&e, &at, &is_lane, &mut budget()).expect("budget");
    assert_eq!(plans.iter().map(|p| p.at.as_str()).collect::<Vec<_>>(), ["amp"]);
}

#[test]
fn an_empty_frozen_list_plans_nothing() {
    // freeze.test.ts "counts a freeze only once its render is on disk": the render's stat is the document's (334), the plan of nothing is here
    let c = run();
    let e = Edges::of(&c);
    assert_eq!(freeze_plans(&e, &[], &is_lane, &mut budget()), Some(vec![]));
}

#[test]
fn a_spent_budget_answers_none_and_never_a_partial_plan() {
    // decision 336, 338: a None is "the budget is spent"
    let c = bus();
    let e = Edges::of(&c);
    let at = [("verb".to_owned(), "r.wav".to_owned())];
    assert_eq!(freeze_plans(&e, &at, &is_lane, &mut Budget::new(2)), None);
    assert_eq!(e.upstream_of("verb", &mut Budget::new(1)), None);
    let inside = e.upstream_of("verb", &mut budget()).expect("budget");
    assert_eq!(e.blocked_by("verb", &inside, &mut Budget::new(1)), None);
}

#[test]
fn the_edges_are_held_once_and_a_name_is_looked_up_not_scanned() {
    // decision 336: a run of 20,000 steps is 20,000 edges, and a hundred upstream walks of it stay fast
    let names: Vec<String> = (0..20_000).map(|i| format!("m{i}")).collect();
    let uses: Vec<&str> = names.iter().map(String::as_str).collect();
    let c = chains(vec![("gtr", chain(Some("gtr"), &uses, &["OUT"]))]);
    let e = Edges::of(&c);
    assert_eq!(e.len(), 20_001);
    fast("a hundred walks", || {
        for _ in 0..100 {
            let set = e.upstream_of("m19999", &mut Budget::new(usize::MAX)).expect("budget");
            assert_eq!(set.len(), 20_001);
        }
    });
}
