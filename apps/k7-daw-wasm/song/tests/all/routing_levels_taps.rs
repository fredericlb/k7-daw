// routing_levels_taps.rs — link levels laid on the derived outs, the unplayed ones named, and a rack's routed taps (decision 109).
// A reader would get wrong that only a link to a bus plays at a level; the compile's send itself is 2h's.

use indexmap::IndexMap;
use song::routing::{Chain, Out, TapFeed, UseStep, apply_link_levels, tap_outs_for, tap_pairs, unplayed_levels};

fn chain(from: Option<&str>, steps: &[&str], outs: &[&str]) -> Chain {
    Chain {
        from: from.map(str::to_owned),
        from_port: None,
        use_steps: steps
            .iter()
            .map(|m| UseStep {
                module: (*m).to_owned(),
                inputs: None,
                enabled: None,
            })
            .collect(),
        out: outs
            .iter()
            .map(|t| Out {
                to: (*t).to_owned(),
                gain: 1.0,
                from_port: None,
            })
            .collect(),
    }
}

fn project() -> IndexMap<String, Chain> {
    IndexMap::from([
        ("gtr".to_owned(), chain(Some("gtr"), &["a1"], &["MIX", "v1"])),
        ("v1".to_owned(), chain(None, &["v1"], &["MIX"])),
    ])
}

fn levels(pairs: &[(&str, f64)]) -> IndexMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), *v)).collect()
}

#[test]
fn a_level_is_the_gain_of_the_send_to_a_bus() {
    // linkLevels.test.ts "is the level of the send to a bus" and "is drawn on the graph's out".
    let mut g = project();
    apply_link_levels(&mut g, &levels(&[("a1>MIX", 1.0), ("a1>v1", 0.3)]));
    assert_eq!(g["gtr"].out[1].gain, 0.3);
    let sink = |t: &str| t == "MIX";
    let lane = |n: &str| n == "gtr";
    assert!(unplayed_levels(&g, &sink, &lane).is_empty());
}

#[test]
fn a_level_on_a_link_to_mix_is_not_played_and_is_named() {
    // linkLevels.test.ts "on a link to MIX is not played, and the graph says so".
    let mut g = project();
    apply_link_levels(&mut g, &levels(&[("a1>MIX", 0.5)]));
    let un = unplayed_levels(&g, &|t| t == "MIX", &|n| n == "gtr");
    assert_eq!(
        (un.len(), un[0].chain.as_str(), un[0].to.as_str(), un[0].gain),
        (1, "gtr", "MIX", 0.5)
    );
}

#[test]
fn a_chain_of_no_steps_levels_from_its_source() {
    // The `<from>` of a level key is the last step, else the source.
    let mut g = IndexMap::from([("l".to_owned(), chain(Some("l"), &[], &["v"]))]);
    apply_link_levels(&mut g, &levels(&[("l>v", 0.2)]));
    assert_eq!(g["l"].out[0].gain, 0.2);
}

fn feed(node: &str, tap: &str, to: &str) -> TapFeed {
    TapFeed {
        node: node.to_owned(),
        tap: tap.to_owned(),
        to: to.to_owned(),
    }
}

#[test]
fn pairs_are_numbered_by_tap_name_from_one() {
    // tapOuts.test.ts "numbers the pairs by tap name, so a rebuild lands every tap where it was" (109).
    let pairs = tap_pairs(&[feed("drums", "Snare", "a"), feed("drums", "Hat", "b")]);
    let got: Vec<(&str, u32)> = pairs["drums"].iter().map(|t| (t.tap.as_str(), t.pair)).collect();
    assert_eq!(got, [("Hat", 1), ("Snare", 2)]);
}

#[test]
fn a_tap_the_node_has_not_is_refused_and_one_it_has_gives_its_note() {
    // tapOuts.test.ts "is refused when the node has no tap of that name" (109).
    let pairs = tap_pairs(&[feed("drums", "Snare", "a")]);
    let outs = tap_outs_for("drums", &pairs["drums"], &|t| (t == "Snare").then_some(38)).expect("routed");
    assert_eq!((outs[0].note, outs[0].pair), (38, 1));
    let bad = tap_pairs(&[feed("drums", "Clap", "a")]);
    assert!(tap_outs_for("drums", &bad["drums"], &|_| None).is_err());
}
