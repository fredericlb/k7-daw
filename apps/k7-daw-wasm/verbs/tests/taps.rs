// taps.rs — a rack node's tap verbs on e2e's rack fixture: the node's file, the compiled kit, undo and the refusals.
// A reader would get wrong where the kit is read: `rack.symbols` in nodes/kit.k7x, compiled onto the beat lane's sampler.

mod support;

use k7x::edit::Value;
use support::Project;

/// The compiled kit on the beat lane: each tap's symbol and note.
fn kit(p: &mut Project) -> Vec<(String, u8)> {
    let t = p.track("beat");
    let pl = t.plugins.iter().find(|pl| pl.rack.is_some()).expect("a sampler");
    pl.rack.iter().flatten().map(|s| (s.symbol.clone(), s.note)).collect()
}

fn symbol(p: &Project, s: &str) -> Value {
    p.data("nodes/kit.k7x")
        .get("rack")
        .and_then(|r| r.get("symbols"))
        .and_then(|m| m.get(s))
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn a_tap_note_moves_the_pad_in_the_node_file() {
    let mut p = Project::open("rack");
    let ran = p.round_trip("node tap note kit SN 40");
    assert_eq!(ran.answer.log, ["kit: SN on note 40"]);
    assert_eq!(ran.answer.effects, ["reload"]);
    assert_eq!(ran.changed(&p.dir), ["nodes/kit.k7x"]);
    p.ok("node tap note kit SN 40");
    assert_eq!(symbol(&p, "SN").get("note"), Some(&Value::Num(40.0)));
    assert!(kit(&mut p).contains(&("SN".to_owned(), 40)));
    p.refused("node tap note kit SN 128", "usage: node tap note");
    p.refused("node tap note kit SN 1.5", "usage: node tap note");
    p.refused("node tap note beat SN 40", "node tap:");
}

#[test]
fn a_tap_sample_is_one_the_picker_offers() {
    let mut p = Project::open("rack");
    let ran = p.round_trip("node tap sample kit HH @project/kick.wav");
    assert_eq!(ran.answer.log, ["kit: HH plays @project/kick.wav"]);
    p.ok("node tap sample kit HH @project/kick.wav");
    assert_eq!(
        symbol(&p, "HH").get("sample"),
        Some(&Value::Str("@project/kick.wav".to_owned()))
    );
    p.refused("node tap sample kit HH @project/ghost.wav", "no '@project/ghost.wav'");
    let ran = p.round_trip("node tap sample kit HH none");
    assert_eq!(ran.answer.log, ["kit: HH has no sample"]);
}

#[test]
fn a_tap_is_added_after_the_highest_on_a_note_or_beside_another() {
    let mut p = Project::open("rack");
    let ran = p.round_trip("node tap add kit CL");
    assert_eq!(ran.answer.log.len(), 1, "{:?}", ran.answer.log);
    assert!(
        ran.answer.log[0].starts_with("kit: tap CL on note "),
        "{:?}",
        ran.answer.log
    );
    p.ok("node tap add kit CL");
    assert!(symbol(&p, "CL").get("note").is_some(), "the tap is in the node's file");
    assert!(
        !kit(&mut p).iter().any(|(s, _)| s == "CL"),
        "a tap with no sample plays nothing"
    );
    p.refused("node tap add kit CL", "node tap add:");
    let ran = p.round_trip("node tap at kit 50 RIM");
    assert_eq!(ran.answer.log, ["kit: tap RIM on note 50 — pick its sample"]);
    p.refused("node tap at kit 36 X", "node tap at:");
    p.refused("node tap at kit low", "<note> expects a number");
    let ran = p.round_trip("node tap insert kit after BD TOM");
    assert!(
        ran.answer.log[0].contains(", after BD — pick its sample"),
        "{:?}",
        ran.answer.log
    );
    p.refused("node tap insert kit beside BD", "usage: node tap insert");
    p.refused("node tap insert kit after GHOST", "node tap insert:");
}

#[test]
fn a_tap_is_renamed_muted_and_removed() {
    let mut p = Project::open("rack");
    let ran = p.round_trip("node tap rename kit HH HAT");
    assert_eq!(ran.answer.log, ["kit: tap HH is now HAT"]);
    p.ok("node tap rename kit HH HAT");
    assert!(kit(&mut p).iter().any(|(s, _)| s == "HAT"));
    let ran = p.round_trip("node tap mute kit BD");
    assert_eq!(ran.answer.log, ["kit: BD muted"]);
    p.ok("node tap mute kit BD");
    assert_eq!(symbol(&p, "BD").get("mute"), Some(&Value::Bool(true)));
    assert!(!kit(&mut p).iter().any(|(s, _)| s == "BD"), "a muted tap is not played");
    let ran = p.ok("node tap mute kit BD");
    assert_eq!(ran.answer.log, ["kit: BD unmuted"]);
    p.refused("node tap mute kit BD maybe", "usage: node tap mute");
    let ran = p.round_trip("node tap remove kit SN");
    assert_eq!(ran.answer.log, ["kit: tap SN removed"]);
    p.ok("node tap remove kit SN");
    assert!(symbol(&p, "SN") == Value::Null);
    p.refused("node tap remove kit SN", "node tap:");
}

#[test]
fn a_tap_chain_is_made_grown_moved_and_emptied() {
    let mut p = Project::open_crate("chain-rack");
    let ran = p.round_trip("node tap chain add kit SN @workspace/audio/compressor");
    assert_eq!(ran.answer.log, ["kit: SN goes through @workspace/audio/compressor"]);
    p.ok("node tap chain add kit SN @workspace/audio/compressor");
    let outs = p
        .track("beat")
        .plugins
        .iter()
        .find_map(|pl| pl.tap_outs.clone())
        .unwrap_or_default();
    assert_eq!(outs.len(), 1, "the snare leaves by an output of its own");
    let ran = p.round_trip("node tap chain add kit SN @workspace/audio/compressor");
    let id = new_node_id(&ran);
    p.ok("node tap chain add kit SN @workspace/audio/compressor");
    let ran = p.round_trip(&format!("node tap chain move kit SN {id} up"));
    assert_eq!(ran.answer.log, ["kit: SN's chain reordered"]);
    p.refused(
        &format!("node tap chain move kit SN {id} sideways"),
        "usage: node tap chain move",
    );
    let ran = p.round_trip(&format!("node tap chain remove kit SN {id}"));
    assert_eq!(ran.answer.log, ["kit: removed from SN's chain"]);
    p.refused("node tap chain remove kit SN nothing", "node tap:");
    p.refused("node tap chain add kit SN @workspace/ghost/none", "node tap:");
    p.refused("node tap out kit SN", "usage: node tap out");
}

/// The id the last write gave a new node in the chain files, read off the ids handed out.
fn new_node_id(ran: &support::Ran) -> String {
    ran.after
        .iter()
        .filter(|(path, b)| ran.before.get(*path) != Some(*b) && path.contains("/chains/"))
        .find_map(|(_, b)| {
            let text = String::from_utf8_lossy(b).into_owned();
            text.split("id(\"")
                .skip(1)
                .filter_map(|s| s.split('"').next())
                .find(|id| id.starts_with("id-"))
                .map(str::to_owned)
        })
        .unwrap_or_default()
}

#[test]
fn a_tap_out_names_a_subchain_or_the_main_output() {
    let mut p = Project::open_crate("chain-rack");
    p.ok("node tap chain add kit SN @workspace/audio/compressor");
    let ran = p.round_trip("node tap out kit SN none");
    assert_eq!(ran.answer.log, ["kit: SN back on the main output"]);
}
