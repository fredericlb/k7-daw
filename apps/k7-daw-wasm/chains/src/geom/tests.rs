// The TypeScript tests of `jacksFor` (jacks.test.ts): the sockets a node shows, and the sizes a panel and a note take.

use super::*;

fn port(name: &str, mandatory: Option<bool>) -> Port {
    Port {
        name: name.into(),
        kind: Signal::Audio,
        mandatory,
    }
}

/// Two audio inputs, the second optional: the crossfade of the song crate's ports page.
fn crossfade() -> Vec<Port> {
    vec![port("a", None), port("b", Some(false))]
}

fn fed(ports: &[&str]) -> HashSet<String> {
    ports.iter().map(|p| (*p).to_owned()).collect()
}

fn jack(name: Option<&str>, kind: Signal, free: bool) -> Jack {
    Jack {
        name: name.map(str::to_owned),
        kind,
        free,
    }
}

#[test]
fn gives_a_module_that_declares_nothing_the_one_socket_every_module_has() {
    assert_eq!(
        jacks_for(None, &fed(&[]), Some(Signal::Midi), true, false, None, 0),
        vec![jack(None, Signal::Midi, true)]
    );
    assert_eq!(
        jacks_for(Some(&[]), &fed(&[""]), None, true, false, None, 0),
        vec![jack(None, Signal::Audio, false)]
    );
}

#[test]
fn shows_the_mandatory_port_alone_until_something_is_wired_to_it() {
    let got = jacks_for(Some(&crossfade()), &fed(&[]), Some(Signal::Audio), true, false, None, 0);
    assert_eq!(got, vec![jack(Some("a"), Signal::Audio, true)]);
}

#[test]
fn reveals_the_next_one_as_soon_as_the_flow_arrives() {
    let got = jacks_for(
        Some(&crossfade()),
        &fed(&[""]),
        Some(Signal::Audio),
        true,
        false,
        None,
        0,
    );
    assert_eq!(
        got,
        vec![
            jack(Some("a"), Signal::Audio, false),
            jack(Some("b"), Signal::Audio, true)
        ]
    );
}

#[test]
fn stops_at_what_the_module_declares() {
    let got = jacks_for(
        Some(&crossfade()),
        &fed(&["", "b"]),
        Some(Signal::Audio),
        true,
        false,
        None,
        0,
    );
    assert_eq!(
        got,
        vec![
            jack(Some("a"), Signal::Audio, false),
            jack(Some("b"), Signal::Audio, false)
        ]
    );
}

#[test]
fn keeps_a_lower_port_visible_when_only_a_higher_one_is_wired() {
    let mut three = crossfade();
    three.push(port("c", Some(false)));
    let got = jacks_for(Some(&three), &fed(&["b"]), Some(Signal::Audio), true, false, None, 0);
    let free: Vec<bool> = got.iter().map(|j| j.free).collect();
    assert_eq!(free, vec![true, false, true]);
}

#[test]
fn draws_every_mandatory_port_at_once_however_many_there_are() {
    let both = vec![port("a", None), port("b", None)];
    let got = jacks_for(Some(&both), &fed(&[]), Some(Signal::Audio), true, false, None, 0);
    let names: Vec<_> = got.iter().map(|j| j.name.clone().unwrap_or_default()).collect();
    assert_eq!(names, vec!["a", "b"]);
}

#[test]
fn shows_all_the_outputs_from_the_start_since_none_of_them_is_progressive() {
    let outs = vec![port("main", None), port("cue", None)];
    let free = |fed_ports: &[&str]| -> Vec<bool> {
        jacks_for(Some(&outs), &fed(fed_ports), Some(Signal::Audio), false, false, None, 0)
            .iter()
            .map(|j| j.free)
            .collect()
    };
    // A cable naming no port is the first output, so `cue` stays free: occupancy is per socket.
    assert_eq!(free(&[""]), vec![false, true]);
    assert_eq!(free(&["cue"]), vec![true, false]);
    assert_eq!(free(&[]), vec![true, true]);
}

#[test]
fn grows_a_spare_socket_on_a_taken_output_and_one_per_bare_cable_on_a_plain_input() {
    let outs = vec![port("audio", None), port("midi", None)];
    let got = jacks_for(Some(&outs), &fed(&["audio"]), None, false, true, Some("audio"), 0);
    // The spare twin of a named cable carries the cable's own name.
    assert_eq!(got.len(), 3);
    assert_eq!(got[1], jack(Some("audio"), Signal::Audio, true));
    let many = jacks_for(None, &fed(&[""]), Some(Signal::Audio), true, false, None, 3);
    assert_eq!(many.len(), 3);
}

#[test]
fn sizes_a_panel_by_its_grid_and_a_plain_card_by_its_constants() {
    assert_eq!(node_size(None, false), (NODE_W, NODE_H));
    assert_eq!(node_size(None, true), (NODE_W, NODE_H + FOOTER_H));
    let ui = Panel::new(3.0, 2.0, vec![("k", crate::panel::Widget::at("knob", 0.0, 0.0))]);
    let (w, h) = node_size(Some(&ui), false);
    assert_eq!(w, NODE_MIN_W.max(UI_PAD * 2.0 + cols_width(3.0)));
    assert_eq!(h, NODE_H.max(UI_CONTENT_TOP + rows_height(2.0) + UI_EDGE));
}

#[test]
fn sizes_a_note_to_its_text_wrapped_past_the_widest_a_note_goes() {
    assert_eq!(note_size(""), (NOTE_MIN_W, NODE_H));
    let (w, h) = note_size(&"x".repeat(200));
    assert_eq!(w, NOTE_MAX_W);
    // 200 glyphs over (320 - 20) / 6.6 = 45 per line is five drawn lines.
    assert_eq!(h, NODE_H.max(5.0 * NOTE_LINE_H + NOTE_PAD * 2.0));
}
