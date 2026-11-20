// lanes_hostile.rs — the passes of 2e on hostile and full-size input, each answered fast (decision 283): the answer is asserted with the time.
// A reader would get wrong that the chains are built by hand: `Inputs` is plain data, so a pass is fed 100,000 chains without 100,000 files.

use crate::lanes_support;
use crate::support;

use lanes_support::{fast, fast_retry, inputs, inputs_with, lane};
use serde_json::json;
use song::lanes::inputs::Inputs;
use song::lanes::live::live_inputs_of;
use song::lanes::ownership::chain_ownership;
use song::lanes::references::{FaultKind, MAX_FAULTS};
use song::routing::{Chain, Out, RoutedTap, UseStep};

const N: usize = 100_000;

fn step(module: &str) -> UseStep {
    UseStep {
        module: module.to_owned(),
        inputs: None,
        enabled: None,
    }
}

fn out(to: &str) -> Out {
    Out {
        to: to.to_owned(),
        gain: 1.0,
        from_port: None,
    }
}

fn chain(from: Option<&str>, steps: &[&str], outs: &[&str]) -> Chain {
    Chain {
        from: from.map(str::to_owned),
        from_port: None,
        use_steps: steps.iter().map(|s| step(s)).collect(),
        out: outs.iter().map(|o| out(o)).collect(),
    }
}

fn base(extra: &[(&str, &str)]) -> Inputs {
    let mut files: Vec<(&str, &str)> = vec![("nodes/mic.k7x", "extends(\"@meta/track\")\n")];
    files.extend_from_slice(extra);
    inputs(&files)
}

// chain_ownership: 100,000 taps over 100,000 chains.
#[test]
fn a_hundred_thousand_taps_over_a_hundred_thousand_chains_stay_linear() {
    let mut i = base(&[]);
    let taps: Vec<RoutedTap> = (0..N)
        .map(|n| RoutedTap {
            node: "kit".into(),
            tap: format!("t{n}"),
            to: format!("r{n}"),
            pair: n as u32 + 1,
        })
        .collect();
    i.routed_taps.insert("kit".into(), taps);
    for n in 0..N {
        i.chains
            .insert(format!("r{n}"), chain(None, &[&format!("r{n}")], &["OUT"]));
    }
    let o = fast("taps over chains", 5, || chain_ownership(&i, &i.lanes()));
    assert_eq!(o.tap_chains.len(), N);
    assert_eq!(o.owner_of("r99999"), Some("r99999"));
    assert!(o.faults.is_empty());
}

// chain_ownership: a node of 100,000 tap_outs of which 50,000 are routed.
#[test]
fn a_node_of_a_hundred_thousand_taps_half_routed_gives_a_capped_fault_list() {
    let outs: String = (0..N).map(|n| format!("t{n}: \"r{n}\",")).collect();
    let kit = format!("plugin({{ type: \"internal\", name: \"Sampler\" }})\ntap_outs({{ {outs} }})\n");
    let mut i = base(&[("nodes/kit.k7x", &kit)]);
    let taps: Vec<RoutedTap> = (0..N / 2)
        .map(|n| RoutedTap {
            node: "kit".into(),
            tap: format!("t{n}"),
            to: format!("r{n}"),
            pair: 1,
        })
        .collect();
    i.routed_taps.insert("kit".into(), taps);
    let o = fast("half-routed taps", 5, || chain_ownership(&i, &i.lanes()));
    assert_eq!(o.faults.len(), MAX_FAULTS + 1);
    assert_eq!(o.faults.last().map(|f| f.kind), Some(FaultKind::More));
    assert!(o.faults[..MAX_FAULTS].iter().all(|f| f.kind == FaultKind::TapUnrouted));
}

// side_input_track asked 100,000 times over a chain of 100,000 steps; a bus fan-in of 100,000.
#[test]
fn a_side_input_and_a_bus_fan_in_at_full_size() {
    let mut i = base(&[]);
    let steps: Vec<String> = (0..N).map(|n| format!("s{n}")).collect();
    let refs: Vec<&str> = steps.iter().map(String::as_str).collect();
    i.chains.insert("long".into(), chain(Some("mic"), &refs, &["OUT"]));
    for n in 0..N {
        i.chains.insert(format!("f{n}"), chain(Some("mic"), &[], &["bus"]));
    }
    i.chains.insert("bus".into(), chain(None, &["verb"], &["OUT"]));
    let o = fast("bus fan-in", 5, || chain_ownership(&i, &i.lanes()));
    assert_eq!(o.bus_chains.iter().collect::<Vec<_>>(), ["bus"]);
    let found = fast("100,000 side inputs", 5, || {
        steps.iter().filter(|s| o.side_input_track(s) == Some("long")).count()
    });
    assert_eq!(found, N);
}

fn rec_module(rig_label: bool) -> String {
    let ch = if rig_label {
        "channel(\"wide\")"
    } else {
        "channel(\"ghost\")"
    };
    format!("io(\"in\")\n{ch}\n")
}

// live_inputs_of: 100,000 input chains each on one label of 64 channels, a label absent, every input on one lane.
#[test]
fn a_hundred_thousand_inputs_on_one_label_and_on_one_lane() {
    let wide: Vec<u32> = (1..=64).collect();
    let rig = json!({"channels": {"in": [{"ch": wide, "label": "wide"}]}});
    let files: Vec<(&str, &str)> = vec![("nodes/mic.k7x", "extends(\"@meta/track\")\n")];
    let rec = rec_module(true);
    let ghost = rec_module(false);
    let mut with: Vec<(&str, &str)> = files.clone();
    with.push(("nodes/rec.k7x", &rec));
    with.push(("nodes/ghost.k7x", &ghost));
    let mut i = inputs_with(&with, &rig);
    for n in 0..N {
        i.chains.insert(format!("in{n}"), chain(None, &["rec"], &["OUT"]));
    }
    let (live, own) = fast("100,000 inputs", 10, || {
        let lanes = i.lanes();
        let own = chain_ownership(&i, &lanes);
        (live_inputs_of(&i, &lanes, &own), own)
    });
    assert_eq!(live.len(), N);
    assert_eq!(live["in0"].channels.len(), 64);
    assert_eq!(own.input_chains.len(), N);
    // A label the rig lacks resolves to no channel; every input run to one lane lets the first win.
    i.chains.clear();
    for n in 0..N {
        i.chains.insert(format!("g{n}"), chain(None, &["ghost"], &["mic"]));
    }
    let live = fast("inputs on one lane", 10, || {
        let lanes = i.lanes();
        let own = chain_ownership(&i, &lanes);
        live_inputs_of(&i, &lanes, &own)
    });
    assert_eq!(live.keys().collect::<Vec<_>>(), ["mic"]);
    assert!(live["mic"].channels.is_empty());
}

// ---- check_references ----

use song::lanes::references::{Checked, check_references};
use song::record::Recorder;
use song::{Source, Timeline};
use std::sync::Arc;
use support::mem;

fn mem_of(files: &[(&str, &str)]) -> song::MemSource {
    let owned = lanes_support::laid(files);
    let refs: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    mem(&refs)
}

fn check(i: &Inputs, source: &dyn Source) -> Checked {
    check_references(source, i, &i.lanes()).expect("checked")
}

fn set_timelines(i: &mut Inputs, list: Vec<(String, serde_json::Value)>) {
    i.timelines.values = list
        .into_iter()
        .map(|(id, v)| (id, Arc::new(serde_json::from_value::<Timeline>(v).expect("a timeline"))))
        .collect();
}

fn tl(events: Vec<serde_json::Value>) -> serde_json::Value {
    json!({"length": "2:0", "lanes": [{"lane": "l", "events": events}]})
}

const KICK: (&str, &str) = ("clips/kick.k7x", "source(\"clips/kick.wav\")\n");
const WAV: (&str, &str) = ("clips/kick.wav", "RIFF");

// A nest of 6 levels of 100 events is 10^12 paths for the TypeScript: each event is visited once here.
#[test]
fn a_nest_of_six_levels_of_a_hundred_events_is_linear() {
    let mut i = inputs(&[KICK, WAV]);
    let src = mem_of(&[KICK, WAV]);
    let list: Vec<_> = (0..6)
        .map(|k| {
            let name = if k < 5 {
                format!("t{}", k + 1)
            } else {
                "kick".to_owned()
            };
            let events = (0..100).map(|n| json!({"ref": name, "start": n + 1})).collect();
            (format!("t{k}"), tl(events))
        })
        .collect();
    set_timelines(&mut i, list);
    let c = fast("six levels", 5, || check(&i, &src));
    assert!(
        c.faults.is_empty(),
        "{:?}",
        c.faults.iter().map(|f| f.key()).collect::<Vec<_>>()
    );
}

// A chain of 100,000 timelines overflows no stack; a cycle of 10,000 is capped; a timeline in itself is one fault.
#[test]
fn a_long_chain_a_big_cycle_and_a_self_loop_end_fast() {
    let mut i = inputs(&[]);
    let src = mem_of(&[]);
    let chain_list: Vec<_> = (0..N)
        .map(|k| {
            let to = if k + 1 < N {
                format!("t{}", k + 1)
            } else {
                "nope".to_owned()
            };
            (format!("t{k}"), tl(vec![json!({"ref": to, "start": 1})]))
        })
        .collect();
    set_timelines(&mut i, chain_list);
    let c = fast("a chain of 100,000 timelines", 5, || check(&i, &src));
    assert_eq!(c.faults.len(), 1);
    assert_eq!(c.faults[0].key(), format!("ref_missing|timeline:t{}:l|nope", N - 1));
    let ring: Vec<_> = (0..10_000)
        .map(|k| {
            (
                format!("t{k}"),
                tl(vec![json!({"ref": format!("t{}", (k + 1) % 10_000), "start": 1})]),
            )
        })
        .collect();
    set_timelines(&mut i, ring);
    let c = fast("a cycle of 10,000", 5, || check(&i, &src));
    assert_eq!(c.faults.len(), MAX_FAULTS + 1);
    assert!(
        c.faults[..MAX_FAULTS]
            .iter()
            .all(|f| f.kind == FaultKind::TimelineCycle)
    );
    set_timelines(&mut i, vec![("x".into(), tl(vec![json!({"ref": "x", "start": 1})]))]);
    let c = check(&i, &src);
    assert_eq!(
        c.faults.iter().map(|f| f.key()).collect::<Vec<_>>(),
        ["timeline_cycle|timeline:x:l|x"]
    );
}

// 100,000 events on absent refs stop at 1,000 faults and one More; 100,000 refs to one broken clip are one fault.
#[test]
fn a_flood_of_faults_is_capped_and_a_repeated_one_is_said_once() {
    let absent: String = (0..N).map(|n| format!("ref(\"a{n}\").start({n}),")).collect();
    let files = [("nodes/big.k7x", lane(&format!("events([{absent}])\n")))];
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let i = inputs(&borrowed);
    let c = fast("100,000 absent refs", 5, || check(&i, &mem_of(&borrowed)));
    assert_eq!(c.faults.len(), MAX_FAULTS + 1);
    assert_eq!(c.faults.last().map(|f| f.kind), Some(FaultKind::More));
    assert_eq!(c.findings.len(), MAX_FAULTS + 1);
    let same: String = (0..N).map(|n| format!("ref(\"kick\").start({n}),")).collect();
    let borrowed = [
        ("nodes/big.k7x", lane(&format!("events([{same}])\n"))),
        ("clips/kick.k7x", "source(\n".into()),
    ];
    let borrowed: Vec<(&str, &str)> = borrowed.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let i = inputs(&borrowed);
    let c = fast("one broken clip 100,000 times", 5, || check(&i, &mem_of(&borrowed)));
    assert_eq!(
        c.faults.iter().map(|f| f.key()).collect::<Vec<_>>(),
        ["clip_broken|track:big|kick"]
    );
}

// 8,000 absent clips are stat'ed through a Recorder, which lists their folder once.
#[test]
fn eight_thousand_absent_clips_are_named_with_one_listing_of_their_folder() {
    let mut owned: Vec<(String, String)> = (0..8_000)
        .map(|n| (format!("clips/c{n}.k7x"), format!("source(\"audio/c{n}.wav\")\n")))
        .collect();
    owned.push(("audio/keep.wav".into(), "x".into()));
    let events: String = (0..8_000).map(|n| format!("ref(\"c{n}\").start({n}),")).collect();
    owned.push(("nodes/big.k7x".into(), lane(&format!("events([{events}])\n"))));
    let borrowed: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let i = inputs(&borrowed);
    let src = mem_of(&borrowed);
    let rec = Recorder::new(&src);
    let c = fast("8,000 absent clips", 10, || check(&i, &rec));
    assert_eq!(c.faults.len(), MAX_FAULTS + 1);
    assert!(c.faults[..MAX_FAULTS].iter().all(|f| f.kind == FaultKind::AudioMissing));
    let bundle = rec.into_bundle(&["/p".to_owned()]);
    assert_eq!(bundle.lists.keys().filter(|k| k.ends_with("/proj/audio")).count(), 1);
}

// 264: a pattern path climbing out, with a NUL or with a backslash is no panic and no wrong fault.
#[test]
fn a_hostile_pattern_path_is_refused_without_a_fault_or_a_panic() {
    let other = ("algo/other.strudel", "s(\"bd\")\n");
    for strudel in ["algo/../../../../../../x.strudel", "algo/\\u0000.strudel"] {
        let node = format!("extends(\"@meta/strudel-track\")\nstrudel(\"{strudel}\")\n");
        let files = [("nodes/kit.k7x", node.as_str()), other];
        let i = inputs(&files);
        let c = check(&i, &mem_of(&files));
        assert!(c.faults.is_empty(), "{strudel}: {:?}", c.faults);
        // The path reaches the check as a lane's own, and the Source or the spelling refuses it.
        assert!(i.lanes().sound.contains_key("kit") && i.stops().is_empty(), "{strudel}");
    }
    // A backslash is spelled as a separator on every platform, so the file is not there.
    let node = "extends(\"@meta/strudel-track\")\nstrudel(\"algo/a\\\\b.strudel\")\n";
    let files = [("nodes/kit.k7x", node), other];
    let i = inputs(&files);
    let c = check(&i, &mem_of(&files));
    assert!(i.lanes().sound.contains_key("kit"));
    assert_eq!(c.faults.len(), 1, "{:?}", c.faults);
}

// 259: a lane called by 2 MB, with 200,000 events on one absent ref, copies its name no more than a fault's cut of it (15).
#[test]
fn a_long_lane_name_is_cut_before_it_is_copied_per_event() {
    let events: String = (0..200_000).map(|n| format!("ref(\"nope\").start({n}),")).collect();
    let files = [("nodes/a.k7x", lane(&format!("events([{events}])\n")))];
    let borrowed: Vec<(&str, &str)> = files.iter().map(|(a, b)| (*a, b.as_str())).collect();
    let mut i = inputs(&borrowed);
    let module = i.local.modules.shift_remove("a").expect("the lane");
    let name = "n".repeat(2 << 20);
    i.local.modules.insert(name, module);
    let c = fast_retry("200,000 events on a long lane", 2, || check(&i, &mem_of(&borrowed)));
    assert_eq!(c.faults.len(), 1);
    assert_eq!(c.findings.len(), 1);
}

// Capacity: 2,000 lanes of 200 events and 20,000 chains of 8 steps pass with no fault; the time is printed and asserted.
#[test]
fn a_real_size_project_is_checked_without_a_fault_in_time() {
    let events: String = (0..200).map(|n| format!("ref(\"kick\").start({n}),")).collect();
    let body = lane(&format!("events([{events}])\n"));
    let mut owned: Vec<(String, String)> = (0..2_000).map(|n| (format!("nodes/l{n}.k7x"), body.clone())).collect();
    owned.push((KICK.0.to_owned(), KICK.1.to_owned()));
    owned.push((WAV.0.to_owned(), WAV.1.to_owned()));
    owned.push((
        "nodes/amp.k7x".into(),
        "plugin({ type: \"internal\", name: \"Chorus\" })\n".into(),
    ));
    let borrowed: Vec<(&str, &str)> = owned.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut i = inputs(&borrowed);
    for n in 0..20_000 {
        let steps: Vec<UseStep> = (0..8).map(|_| step("amp")).collect();
        i.chains.insert(
            format!("c{n}"),
            Chain {
                from: Some(format!("l{}", n % 2_000)),
                from_port: None,
                use_steps: steps,
                out: vec![out("OUT")],
            },
        );
    }
    let src = mem_of(&borrowed);
    let t = std::time::Instant::now();
    let c = check(&i, &src);
    let took = t.elapsed();
    eprintln!("capacity: check_references over 2,000 lanes and 20,000 chains took {took:?}");
    assert!(
        c.faults.is_empty(),
        "{:?}",
        c.faults.iter().take(3).map(|f| f.key()).collect::<Vec<_>>()
    );
    assert!(
        took.as_secs() < if cfg!(target_env = "musl") { 30 } else { 5 },
        "{took:?}"
    );
}
