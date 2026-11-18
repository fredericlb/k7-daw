// lanes_wiring.rs — `check_chain_inputs` and `source_type` (decisions 268, 269): the sources a step is fed against its declared ports.
// A reader would get wrong that nothing here has a TypeScript test of its own: the cases are written from the code, one per source kind.

use crate::lanes_support;

use indexmap::IndexMap;
use lanes_support::{fast, inputs, lane};
use song::lanes::inputs::Inputs;
use song::lanes::wiring::{Signal, SignalMemo, WiringInput, check_chain_inputs, source_type};
use song::modules::{Port, PortType};
use song::routing::{Chain, Out, UseStep};
use std::collections::{HashMap, HashSet};

fn port(name: &str, kind: PortType, mandatory: Option<bool>) -> Port {
    Port {
        name: name.to_owned(),
        kind,
        mandatory,
    }
}

fn step(module: &str, inputs: &[&str]) -> UseStep {
    UseStep {
        module: module.to_owned(),
        inputs: (!inputs.is_empty()).then(|| inputs.iter().map(|s| (*s).to_owned()).collect()),
        enabled: None,
    }
}

fn chain(from: Option<&str>, steps: Vec<UseStep>, outs: &[&str]) -> Chain {
    Chain {
        from: from.map(str::to_owned),
        from_port: None,
        use_steps: steps,
        out: outs
            .iter()
            .map(|to| Out {
                to: (*to).to_owned(),
                gain: 1.0,
                from_port: None,
            })
            .collect(),
    }
}

fn world() -> Inputs {
    inputs(&[
        ("clips/kick.k7x", "source(\"clips/kick.wav\")\n"),
        ("clips/take.k7x", "source(\"clips/take.mp4\")\n"),
        ("clips/subs.k7x", "source(\"clips/subs.srt\")\n"),
        ("sequences/riff.k7x", "notes([t(0).d(1).k(60)])\n"),
        ("nodes/aud.k7x", &lane("events([ref(\"kick\").start(\"1:1\")])\n")),
        ("nodes/mid.k7x", &lane("events([ref(\"riff\").start(\"1:1\")])\n")),
        (
            "nodes/mixed.k7x",
            &lane("events([ref(\"kick\").start(\"1:1\"), ref(\"riff\").start(\"2:1\")])\n"),
        ),
        ("nodes/lost.k7x", &lane("events([ref(\"nope\").start(\"1:1\")])\n")),
        ("nodes/empty.k7x", &lane("")),
        ("nodes/grp.k7x", "extends(\"@meta/group-track\")\ntracks([\"aud\"])\n"),
        ("nodes/pic.k7x", "extends(\"@meta/picture-track\")\n"),
        (
            "nodes/kit.k7x",
            "extends(\"@meta/strudel-track\")\nstrudel(\"algo/x.strudel\")\n",
        ),
    ])
}

/// The keys the check gives over `chains`, the modules' ports given by name, tap-fed heads by name.
fn run(i: &Inputs, chains: IndexMap<String, Chain>, ports: &[(&str, Vec<Port>)], tap_fed: &[&str]) -> Vec<String> {
    let lanes = i.lanes();
    let by_module: HashMap<&str, Vec<Port>> = ports.iter().map(|(n, p)| (*n, p.clone())).collect();
    let clips: HashMap<&str, &song::Clip> = i.clips.values.iter().map(|(k, c)| (k.as_str(), &**c)).collect();
    let seqs: HashMap<&str, &song::Sequence> = i.sequences.values.iter().map(|(k, c)| (k.as_str(), &**c)).collect();
    let inputs_of = |n: &str| by_module.get(n).map(Vec::as_slice);
    let picture = |n: &str| lanes.picture.contains_key(n);
    let tap: HashSet<String> = tap_fed.iter().map(|s| (*s).to_owned()).collect();
    let w = WiringInput {
        chains: &chains,
        inputs_of: &inputs_of,
        sound: &lanes.sound,
        clips: &clips,
        sequences: &seqs,
        picture: &picture,
        tap_fed: &tap,
    };
    let mut keys: Vec<String> = check_chain_inputs(&w).iter().map(|f| f.key()).collect();
    keys.sort();
    keys
}

fn one(name: &str, c: Chain) -> IndexMap<String, Chain> {
    std::iter::once((name.to_owned(), c)).collect()
}

fn audio_in() -> Vec<Port> {
    vec![port("in", PortType::Audio, None)]
}

// Every source kind has a type: a sequence is midi, a clip audio or video by its extension, a lane by its events and port.
#[test]
fn each_source_has_the_type_its_kind_gives_it() {
    let i = world();
    let lanes = i.lanes();
    let clips: HashMap<&str, &song::Clip> = i.clips.values.iter().map(|(k, c)| (k.as_str(), &**c)).collect();
    let seqs: HashMap<&str, &song::Sequence> = i.sequences.values.iter().map(|(k, c)| (k.as_str(), &**c)).collect();
    let picture = |n: &str| lanes.picture.contains_key(n);
    let inputs_of = |_: &str| None;
    let chains = IndexMap::new();
    let tap = HashSet::new();
    let w = WiringInput {
        chains: &chains,
        inputs_of: &inputs_of,
        sound: &lanes.sound,
        clips: &clips,
        sequences: &seqs,
        picture: &picture,
        tap_fed: &tap,
    };
    let mut memo = SignalMemo::new();
    let mut t = |id: &str| source_type(id, &w, &mut memo);
    assert_eq!(t("riff"), Some(Signal::Midi));
    assert_eq!(
        (t("kick"), t("take"), t("subs")),
        (Some(Signal::Audio), Some(Signal::Video), Some(Signal::Video))
    );
    assert_eq!((t("aud"), t("mid")), (Some(Signal::Audio), Some(Signal::Midi)));
    // A detyped lane with a port says what it carries; a mixed one, a lane of an unknown ref and an empty one say nothing.
    assert_eq!(
        (t("mixed#midi"), t("mixed#audio")),
        (Some(Signal::Midi), Some(Signal::Audio))
    );
    assert_eq!((t("mixed"), t("lost"), t("empty"), t("kit")), (None, None, None, None));
    assert_eq!(t("grp"), None);
    assert_eq!(
        (t("pic"), t("pic#audio"), t("pic#video")),
        (Some(Signal::Video), Some(Signal::Audio), Some(Signal::Video))
    );
    assert_eq!(t("ghost"), None);
}

// A mandatory port short is too few; more sources than ports is too many; the module's name is the target.
#[test]
fn too_few_and_too_many_are_counted_against_the_ports() {
    let i = world();
    let two = vec![
        port("a", PortType::Audio, None),
        port("b", PortType::Audio, Some(false)),
    ];
    // No flow and no `in`: nothing feeds the first step of a laneless chain.
    let c = one("c", chain(None, vec![step("fx", &[])], &["OUT"]));
    assert_eq!(run(&i, c, &[("fx", two.clone())], &[]), ["too_few|chain:c:1|fx"]);
    // The flow plus one `in` fills both ports.
    let ok = one("c", chain(Some("aud"), vec![step("fx", &["kick"])], &["OUT"]));
    assert!(run(&i, ok, &[("fx", two.clone())], &[]).is_empty());
    let many = one("c", chain(Some("aud"), vec![step("fx", &["kick", "kick"])], &["OUT"]));
    assert_eq!(run(&i, many, &[("fx", two)], &[]), ["too_many|chain:c:1|fx"]);
    // A later step has a flow from the previous one.
    let later = one(
        "c",
        chain(Some("aud"), vec![step("fx", &[]), step("fx", &[])], &["OUT"]),
    );
    assert!(run(&i, later, &[("fx", audio_in())], &[]).is_empty());
}

// A positional type difference where the type is known; unknown types and a "(flow)" are never a difference.
#[test]
fn a_type_difference_is_a_fault_only_where_the_type_is_known() {
    let i = world();
    let c = one("c", chain(Some("mid"), vec![step("fx", &[])], &["OUT"]));
    assert_eq!(run(&i, c, &[("fx", audio_in())], &[]), ["port_type|chain:c:1|in"]);
    let from_port = {
        let mut ch = chain(Some("mixed"), vec![step("fx", &[])], &["OUT"]);
        ch.from_port = Some("audio".into());
        one("c", ch)
    };
    assert!(run(&i, from_port, &[("fx", audio_in())], &[]).is_empty());
    let unknown = one("c", chain(Some("mixed"), vec![step("fx", &[])], &["OUT"]));
    assert!(run(&i, unknown, &[("fx", audio_in())], &[]).is_empty());
    let side = one("c", chain(Some("aud"), vec![step("fx", &["riff"])], &["OUT"]));
    let two = vec![port("a", PortType::Audio, None), port("b", PortType::Audio, None)];
    assert_eq!(run(&i, side, &[("fx", two)], &[]), ["port_type|chain:c:1|b"]);
}

// A group carries no signal: a chain from one is a fault, whatever the chain holds.
#[test]
fn a_group_as_the_source_is_a_fault() {
    let i = world();
    let c = one("c", chain(Some("grp"), vec![step("fx", &[])], &["OUT"]));
    assert_eq!(run(&i, c, &[("fx", audio_in())], &[]), ["group_source|chain:c|grp"]);
}

// A tap-fed head is audio; a fade's shot is video, only at index 0 with no `from`, nothing routed in, a video first port and an out on a picture lane.
#[test]
fn a_tap_fed_head_is_audio_and_a_shot_is_video() {
    let i = world();
    let c = || one("snare-fx", chain(None, vec![step("fx", &[])], &["OUT"]));
    assert!(run(&i, c(), &[("fx", audio_in())], &["fx"]).is_empty());
    assert_eq!(
        run(&i, c(), &[("fx", audio_in())], &[]),
        ["too_few|chain:snare-fx:1|fx"]
    );
    let video = vec![port("in", PortType::Video, None)];
    assert_eq!(
        run(&i, c(), &[("fx", video.clone())], &["fx"]),
        ["port_type|chain:snare-fx:1|in"]
    );
    let shot = one("s", chain(None, vec![step("wipe", &[])], &["pic"]));
    assert!(run(&i, shot, &[("wipe", video.clone())], &[]).is_empty());
    // Routed into by another chain, the head has a flow; with no picture out it has none.
    let no_shot = one("s", chain(None, vec![step("wipe", &[])], &["OUT"]));
    assert_eq!(
        run(&i, no_shot, &[("wipe", video.clone())], &[]),
        ["too_few|chain:s:1|wipe"]
    );
    let mut routed: IndexMap<String, Chain> = one("s", chain(None, vec![step("wipe", &[])], &["OUT"]));
    routed.insert("f".into(), chain(Some("aud"), vec![], &["s"]));
    assert!(run(&i, routed, &[("wipe", video)], &[]).is_empty());
}

// A parked chain, a bypassed step, an unknown module and a module with no ports are skipped.
#[test]
fn what_is_parked_bypassed_unknown_or_unconstrained_is_skipped() {
    let i = world();
    let parked = one("p", chain(None, vec![step("fx", &[])], &[]));
    assert!(run(&i, parked, &[("fx", audio_in())], &[]).is_empty());
    let mut off = step("fx", &[]);
    off.enabled = Some(false);
    let bypass = one("c", chain(None, vec![off], &["OUT"]));
    assert!(run(&i, bypass, &[("fx", audio_in())], &[]).is_empty());
    let unknown = one("c", chain(None, vec![step("ghost", &[])], &["OUT"]));
    assert!(run(&i, unknown, &[], &[]).is_empty());
    let free = one("c", chain(None, vec![step("fx", &[])], &["OUT"]));
    assert!(run(&i, free, &[("fx", Vec::new())], &[]).is_empty());
}

// 268 / 269: a module of 5,000 ports stepped 100,000 times, and a lane of 100,000 events named by 100,000 ports, stay linear.
#[test]
fn a_hostile_wiring_is_linear() {
    let ports: Vec<Port> = (0..5_000)
        .map(|n| port(&format!("p{n}"), PortType::Audio, Some(false)))
        .collect();
    let i = world();
    let mut chains = IndexMap::new();
    for n in 0..100_000 {
        chains.insert(format!("c{n}"), chain(Some("aud"), vec![step("big", &[])], &["OUT"]));
    }
    let keys = fast("a module of 5,000 ports", 5, || run(&i, chains, &[("big", ports)], &[]));
    assert!(keys.is_empty());
    let events: String = (0..100_000).map(|n| format!("ref(\"kick\").start({n}),")).collect();
    let i = inputs(&[
        ("clips/kick.k7x", "source(\"clips/kick.wav\")\n"),
        ("nodes/long.k7x", &lane(&format!("events([{events}])\n"))),
    ]);
    let ins: Vec<&str> = std::iter::repeat_n("long", 100_000).collect();
    let c = one("c", chain(Some("long"), vec![step("fx", &ins)], &["OUT"]));
    let many: Vec<Port> = (0..100_000)
        .map(|n| port(&format!("p{n}"), PortType::Midi, Some(false)))
        .collect();
    // The lane carries nothing it can be told to: every port past the first is a source of an unknown type.
    let keys = fast("100,000 ports naming one lane", 5, || run(&i, c, &[("fx", many)], &[]));
    // At most 1,000 faults and the one `More` (260); the lane was walked once.
    assert_eq!(keys.len(), 1_001, "{}", keys.len());
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("port_type|chain:c:1|")).count(),
        999
    );
    assert!(keys.contains(&"too_many|chain:c:1|fx".to_owned()) && keys.contains(&"more|file:|".to_owned()));
}

// 290: 5,000 chains with no step fed by a group are capped like every other fault, with the one `More`.
#[test]
fn group_sources_are_capped_like_the_other_faults() {
    let i = world();
    let mut chains = IndexMap::new();
    for n in 0..5_000 {
        chains.insert(format!("c{n}"), chain(Some("grp"), vec![], &["OUT"]));
    }
    let keys = fast("5,000 group sources", 5, || run(&i, chains, &[], &[]));
    assert_eq!(keys.len(), 1_001, "{}", keys.len());
    assert_eq!(keys.iter().filter(|k| k.starts_with("group_source|")).count(), 1_000);
    assert_eq!(keys.last().map(String::as_str), Some("more|file:|"));
}

// A ring of 100,000 chains, each routed into the next, is one linear pass: every head has its flow.
#[test]
fn a_ring_of_a_hundred_thousand_chains_is_linear() {
    let i = world();
    let mut chains = IndexMap::new();
    for n in 0..100_000 {
        chains.insert(
            format!("c{n}"),
            chain(None, vec![step("fx", &[])], &[&format!("c{}", (n + 1) % 100_000)]),
        );
    }
    let keys = fast("a ring of 100,000 chains", 5, || {
        run(&i, chains, &[("fx", audio_in())], &[])
    });
    assert!(keys.is_empty(), "{keys:?}");
}
