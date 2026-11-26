//! audio_policy.rs — the rules stated in the test names: there is one list, and the first entry present wins.

use k7_supervisor::audio_policy::{desired_switch, menu_signature, signature};
use k7_supervisor::protocol::AudioDevice;

fn dev(name: &str, current: bool) -> AudioDevice {
    AudioDevice {
        name: name.into(),
        device_type: "CoreAudio".into(),
        current,
        ..AudioDevice::default()
    }
}

fn rank(names: &[&str]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

#[test]
fn it_stays_put_on_the_best_present() {
    let d = [dev("Scarlett", true), dev("Built-in", false)];
    assert_eq!(desired_switch(&d, &rank(&["Scarlett", "Built-in"])), None);
}

#[test]
fn it_returns_when_the_better_one_reappears() {
    let d = [dev("Scarlett", false), dev("Built-in", true)];
    assert_eq!(
        desired_switch(&d, &rank(&["Scarlett", "Built-in"])),
        Some("Scarlett")
    );
}

#[test]
fn it_goes_down_the_list_when_one_is_unplugged() {
    let d = [dev("Built-in", false), dev("HDMI", true)];
    assert_eq!(
        desired_switch(&d, &rank(&["Scarlett", "Built-in"])),
        Some("Built-in")
    );
}

#[test]
fn it_takes_the_first_present_entry() {
    let d = [
        dev("Scarlett", false),
        dev("Built-in", false),
        dev("HDMI", true),
    ];
    assert_eq!(
        desired_switch(&d, &rank(&["Scarlett", "Built-in", "HDMI"])),
        Some("Scarlett")
    );
}

#[test]
fn it_leaves_an_unranked_device_alone() {
    assert_eq!(
        desired_switch(&[dev("HDMI", true)], &rank(&["Scarlett", "Built-in"])),
        None
    );
    assert_eq!(desired_switch(&[dev("HDMI", false)], &[]), None);
}

#[test]
fn it_ranks_one_even_when_nothing_is_current() {
    let d = [dev("Built-in", false), dev("HDMI", false)];
    assert_eq!(desired_switch(&d, &rank(&["Built-in"])), Some("Built-in"));
}

#[test]
fn it_never_acts_on_an_empty_list() {
    assert_eq!(desired_switch(&[], &rank(&["Scarlett"])), None);
}

#[test]
fn a_reorder_moves_the_menu_signature_but_not_the_set_one() {
    let d = [dev("A", true), dev("B", false)];
    assert_ne!(
        menu_signature(&d, &rank(&["A", "B"])),
        menu_signature(&d, &rank(&["B", "A"]))
    );
    assert_eq!(signature(&d), signature(&[dev("B", true), dev("A", false)]));
}

#[test]
fn an_input_moving_alone_moves_the_menu_signature() {
    // The output stays on A: only the window's "input only" line changes, and it must be redrawn.
    let mut split = vec![dev("A", true), dev("B", false)];
    split[1].current_input = true;
    let mut whole = vec![dev("A", true), dev("B", false)];
    whole[0].current_input = true;
    assert_ne!(
        menu_signature(&split, &rank(&["A"])),
        menu_signature(&whole, &rank(&["A"]))
    );
}

#[test]
fn an_empty_list_still_carries_the_ranking_in_the_menu_signature() {
    assert_eq!(menu_signature(&[], &rank(&["x", "y"])), "\u{1e}x\u{1f}y");
}
