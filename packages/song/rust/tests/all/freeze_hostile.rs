// freeze_hostile.rs — hostile input to the freeze graph, the cut and the automatic freezes, each answered fast with its answer asserted.
// A reader would get wrong that the row counts the OUTPUT: 10,000 VSTs on one run are 10^8 edges to walk from a few hundred KB of text.

use crate::freeze_support;
use crate::support;

use freeze_support::{Scene, chain, chains, doc, fast, track, vst};
use song::clips::Budget;
use song::freeze::{BUDGET, Edges, freeze_plans};
use song::routing::{Chain, UseStep};
use song::{Cache, DiskSource};
use support::{Tmp, spell};

fn budget() -> Budget {
    Budget::new(BUDGET)
}

fn names(prefix: &str, n: usize) -> Vec<String> {
    (0..n).map(|i| format!("{prefix}{i}")).collect()
}

fn refs(v: &[String]) -> Vec<&str> {
    v.iter().map(String::as_str).collect()
}

#[test]
fn a_run_of_a_hundred_thousand_steps_is_walked_once() {
    // freeze graph row; decision 336
    let steps = names("m", 100_000);
    let c = chains(vec![("gtr", chain(Some("gtr"), &refs(&steps), &["OUT"]))]);
    let e = Edges::of(&c);
    let up = fast("a long run", || e.upstream_of("m99999", &mut budget())).expect("within the budget");
    assert_eq!(up.len(), 100_001);
    assert_eq!(e.upstream_of("m99999", &mut Budget::new(50_000)), None);
}

#[test]
fn ten_thousand_vsts_on_one_run_stop_at_the_budget_with_no_automatic_freeze() {
    // freeze graph row; decisions 340, 348: 10^8 edges from a small file
    let steps = names("v", 10_000);
    let mut s = Scene::standard("/p", Some("/r"), false);
    s.chains = chains(vec![("gtr", chain(Some("gtr"), &refs(&steps), &["OUT"]))]);
    s.modules = steps.iter().map(|n| (n.clone(), vst(n))).collect();
    s.live = doc(vec![track("gtr", &refs(&steps))]);
    let mut spent = budget();
    let points = fast("ten thousand VSTs", || s.points(&mut spent));
    assert_eq!(points, None);
    assert!(spent.capped());
    let t = Tmp::new("hostile-vsts");
    let src = DiskSource::new(&[spell(&t.0)]).expect("source");
    let out = fast("the freezes", || s.run(&src, &mut Cache::new(), &mut budget()));
    assert!(out.capped && out.freezes.is_empty());
}

#[test]
fn a_thousand_vsts_fanning_into_one_bus_are_one_point_and_not_capped() {
    // freeze graph row; decisions 337, 340
    let lanes = names("l", 1_000);
    let vsts = names("v", 1_000);
    let mut items: Vec<(&str, Chain)> = Vec::new();
    for (l, v) in lanes.iter().zip(&vsts) {
        items.push((l, chain(Some(l), &[v], &["bus"])));
    }
    items.push(("bus", chain(None, &["bus"], &["OUT"])));
    let mut s = Scene::standard("/p", None, false);
    s.chains = chains(items);
    s.modules = vsts.iter().map(|n| (n.clone(), vst(n))).collect();
    s.modules.insert("bus".into(), vst("Bus"));
    s.lanes = lanes.iter().cloned().collect();
    let mut spent = budget();
    let points = fast("a fan-in", || s.points(&mut spent));
    assert_eq!(points, Some(vec!["bus".to_owned()]));
    assert!(!spent.capped(), "{}", spent.spent());
}

#[test]
fn a_cycle_through_ten_thousand_steps_is_walked_without_looping() {
    // freeze graph row; decision 336
    let steps = names("m", 10_000);
    let c = chains(vec![("a", chain(None, &refs(&steps), &["m0"]))]);
    let e = Edges::of(&c);
    let up = fast("a cycle", || e.upstream_of("m5000", &mut budget())).expect("budget");
    assert_eq!(up.len(), 10_000);
    let blocked = e.blocked_by("m5000", &up, &mut budget()).expect("budget");
    assert_eq!(blocked, None);
}

#[test]
fn a_hundred_thousand_frozen_names_on_no_chain_plan_nothing() {
    // freeze graph row; decision 338
    let c = chains(vec![("gtr", chain(Some("gtr"), &["amp"], &["OUT"]))]);
    let e = Edges::of(&c);
    let frozen: Vec<(String, String)> = (0..100_000)
        .map(|i| (format!("ghost{i}"), "r.wav".to_owned()))
        .collect();
    let plans = fast("ghosts", || freeze_plans(&e, &frozen, &|_| false, &mut budget()));
    assert_eq!(plans, Some(vec![]));
}

#[test]
fn a_hundred_thousand_ghosts_past_a_small_budget_answer_none() {
    // freeze graph row; decision 348: each name looked at is charged
    let c = chains(vec![("gtr", chain(Some("gtr"), &["amp"], &["OUT"]))]);
    let e = Edges::of(&c);
    let frozen: Vec<(String, String)> = (0..100_000)
        .map(|i| (format!("ghost{i}"), "r.wav".to_owned()))
        .collect();
    assert_eq!(freeze_plans(&e, &frozen, &|_| false, &mut Budget::new(1_000)), None);
}

#[test]
fn a_frozen_name_on_a_chain_with_a_hundred_thousand_ins_is_one_walk() {
    // freeze graph row; decision 336: the step's side inputs are edges, read once
    let ins = names("s", 100_000);
    let mut c = chain(Some("gtr"), &["amp"], &["OUT"]);
    c.use_steps[0] = UseStep {
        module: "amp".into(),
        inputs: Some(ins.clone()),
        enabled: None,
    };
    let chains = chains(vec![("gtr", c)]);
    let e = Edges::of(&chains);
    let at = [("amp".to_owned(), "r.wav".to_owned())];
    let plans = fast("many ins", || freeze_plans(&e, &at, &|n| n == "gtr", &mut budget())).expect("budget");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].lanes.len(), 1);
    assert_eq!(plans[0].modules.len(), 100_001);
}

#[test]
fn a_cut_of_a_plan_of_nothing_is_empty_and_fast() {
    // freeze_cut row, "a plan of nothing"; decision 342
    let tracks: Vec<_> = names("t", 10_000).iter().map(|n| track(n, &["x"])).collect();
    let d = doc(tracks);
    let plan = song::freeze::FreezePlan {
        at: "x".into(),
        render: String::new(),
        lanes: Default::default(),
        modules: Default::default(),
    };
    let cut = fast("a plan of nothing", || {
        song::freeze::freeze_cut(&d, &plan, &mut budget())
    })
    .expect("budget");
    assert!(cut.tracks.is_empty());
}

#[test]
fn a_vst_named_with_separators_gets_a_render_name_without_them() {
    // automatic freezes row: a VST named `a:b/c\d`, on a lane of its own so that it is a freeze point
    let t = Tmp::new("hostile-names");
    let src = DiskSource::new(&[spell(&t.0)]).expect("source");
    let mut s = Scene::standard(&t.at("proj"), Some(&t.at("renders")), false);
    s.modules.insert("a:b/c\\d".into(), vst("Odd"));
    s.chains
        .insert("odd".into(), chain(Some("odd"), &["a:b/c\\d"], &["OUT"]));
    s.lanes.insert("odd".into());
    s.live.tracks.push(track("odd", &["a:b/c\\d"]));
    let out = fast("separators", || s.run(&src, &mut Cache::new(), &mut budget()));
    assert!(!out.capped);
    let odd = out
        .freezes
        .iter()
        .find(|f| f.module == "a:b/c\\d")
        .expect("the odd VST is frozen");
    let rel = odd.render.strip_prefix(&t.at("renders/")).expect("under renders");
    assert!(rel.starts_with("a-b-c-d-") && rel.ends_with(".wav"), "{rel}");
    for f in &out.freezes {
        let rel = f.render.strip_prefix(&t.at("renders/")).expect("under renders");
        assert!(!rel.contains('/') && !rel.contains('\\'), "{rel}");
    }
}

#[test]
fn a_lane_of_a_megabyte_in_every_plan_is_paid_by_its_bytes() {
    // freeze graph row; decision 348: a plan pays for each name it copies, not only for how many
    let big = "k".repeat(1 << 20);
    let steps = names("m", 300);
    let items: Vec<(String, Chain)> = steps
        .iter()
        .map(|m| (format!("c-{m}"), chain(Some(&big), &[m.as_str()], &["OUT"])))
        .collect();
    let c = chains(items.iter().map(|(k, v)| (k.as_str(), v.clone())).collect());
    let e = Edges::of(&c);
    let at: Vec<(String, String)> = steps.iter().map(|m| (m.clone(), "r.wav".to_owned())).collect();
    let few = freeze_plans(&e, &at[..20], &|n| n == big, &mut budget()).expect("twenty fit");
    assert_eq!(few.len(), 20);
    assert_eq!(
        fast("megabytes", || freeze_plans(&e, &at, &|n| n == big, &mut budget())),
        None
    );
}
