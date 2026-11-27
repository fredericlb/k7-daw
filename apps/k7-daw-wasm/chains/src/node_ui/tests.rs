// The TypeScript tests of the built-in panels (inputUI.test.ts, camera.test.ts, renderCard.test.ts, a rack's in buildLanes.test.ts).

use super::*;

fn mono(n: f64) -> InputDevice {
    InputDevice {
        name: format!("Input {n}"),
        channels: vec![n],
    }
}

fn rig() -> Vec<InputDevice> {
    (1..=4).map(|n| mono(n as f64)).collect()
}

fn options<'a>(ui: &'a Panel, id: &str) -> &'a [WidgetOption] {
    ui.widgets[id].options.as_deref().unwrap_or_default()
}

fn args(ui: &Panel, id: &str) -> Vec<String> {
    options(ui, id)
        .iter()
        .map(|o| o.arg.clone().unwrap_or_default())
        .collect()
}

#[test]
fn the_socket_dropdown_offers_the_interfaces_mono_inputs_for_a_mono_module() {
    let ui = input_ui(&rig(), Some(&[2.0]));
    let labels: Vec<_> = options(&ui, "channel").iter().map(|o| o.label.as_str()).collect();
    assert_eq!(labels, ["Input 1", "Input 2", "Input 3", "Input 4"]);
    // What gets written, which is the half that matters.
    assert_eq!(args(&ui, "channel"), ["1", "2", "3", "4"]);
}

#[test]
fn the_socket_dropdown_offers_the_stereo_devices_for_a_stereo_module() {
    let mut devices = vec![InputDevice {
        name: "Input 1 + 2".into(),
        channels: vec![1.0, 2.0],
    }];
    devices.push(mono(3.0));
    let ui = input_ui(&devices, Some(&[1.0, 2.0]));
    assert_eq!(options(&ui, "channel").len(), 1);
    assert_eq!(args(&ui, "channel"), ["1,2"]);
}

#[test]
fn the_socket_dropdown_is_empty_without_throwing_when_no_interface_is_answering() {
    let ui = input_ui(&[], Some(&[2.0]));
    assert!(options(&ui, "channel").is_empty());
    assert_eq!(options(&ui, "width").len(), 2);
    assert_eq!(options(&ui, "monitor").len(), 3);
}

#[test]
fn the_width_dropdown_writes_the_channel_field_too() {
    let ui = input_ui(&rig(), Some(&[2.0]));
    assert_eq!(ui.widgets["width"].bind.as_deref(), Some("@channel"));
    assert_eq!(args(&ui, "width"), ["2", "1,2"]);
}

#[test]
fn the_width_pairs_on_the_even_odd_boundary_from_either_half() {
    assert_eq!(args(&input_ui(&rig(), Some(&[4.0])), "width")[1], "3,4");
    assert_eq!(args(&input_ui(&rig(), Some(&[3.0])), "width")[1], "3,4");
}

#[test]
fn the_width_narrows_a_stereo_module_back_to_its_first_channel() {
    assert_eq!(args(&input_ui(&rig(), Some(&[3.0, 4.0])), "width")[0], "3");
}

#[test]
fn the_width_assumes_channel_one_for_an_input_that_names_none() {
    assert_eq!(args(&input_ui(&rig(), None), "width"), ["1", "1,2"]);
}

#[test]
fn an_input_panel_binds_every_widget_to_a_module_field_and_offers_no_gain_knob() {
    let ui = input_ui(&rig(), Some(&[1.0]));
    for w in ui.widgets.values() {
        assert!(w.bind.as_deref().is_some_and(|b| b.starts_with('@')));
        assert_eq!(w.kind, "dropdown");
    }
    let mut keys: Vec<_> = ui.widgets.keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["channel", "monitor", "width"]);
}

#[test]
fn a_camera_offers_every_lens_writing_the_label_and_default_first_and_always() {
    let here = vec!["FaceTime HD Camera".to_owned(), "Logitech BRIO".to_owned()];
    let ui = camera_ui(&here);
    let w = &ui.widgets["camera"];
    assert_eq!((w.kind.as_str(), w.bind.as_deref()), ("dropdown", Some(CAMERA_BIND)));
    let labels: Vec<_> = options(&ui, "camera").iter().map(|o| o.label.clone()).collect();
    assert_eq!(labels, ["default", "FaceTime HD Camera", "Logitech BRIO"]);
    assert_eq!(args(&ui, "camera"), labels);
    assert_eq!(
        options(&camera_ui(&[]), "camera"),
        [option_arg("default", 0.0, "default")]
    );
}

#[test]
fn a_midi_input_offers_any_first_and_each_device() {
    let ui = midi_in_ui(&["Arturia KeyStep 37".to_owned()]);
    assert_eq!(args(&ui, "device"), ["any", "Arturia KeyStep 37"]);
    assert_eq!(ui.widgets["device"].bind.as_deref(), Some(DEVICE_BIND));
}

#[test]
fn the_render_card_offers_exactly_the_two_shapes_as_a_dropdown() {
    let ui = render_ui();
    let w = &ui.widgets["aspect"];
    assert_eq!(
        (w.kind.as_str(), w.bind.as_deref()),
        ("dropdown", Some("@project:aspect"))
    );
    let rows: Vec<_> = options(&ui, "aspect")
        .iter()
        .map(|o| (o.label.as_str(), o.value))
        .collect();
    // The values are the row indices the card carries, or the menu ticks the wrong row.
    assert_eq!(rows, [("16:9", 0.0), ("9:16", 1.0)]);
}

fn kit() -> Vec<RackSound> {
    [("BD", 36.0), ("HH", 42.0), ("SN", 38.0)]
        .into_iter()
        .map(|(symbol, note)| RackSound {
            symbol: symbol.into(),
            note,
        })
        .collect()
}

#[test]
fn a_rack_draws_an_octave_as_a_4x3_grid_the_lowest_note_bottom_left_an_empty_note_an_add_tap() {
    let ui = rack_ui(&kit(), false);
    let tabs = ui.tabs.as_ref().expect("two octaves");
    // 36..47 holds the sounds; 48..59 is offered above them, so a rack can grow upward.
    assert_eq!(tabs.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["C2", "C3"]);
    assert_eq!((ui.cols, ui.rows), (PADS_ACROSS * TAP_COLS, PADS_UP * TAP_ROWS));
    let c2 = &tabs[0].widgets;
    assert_eq!(c2.len(), 12);
    // C (36) is the first cell of the bottom row, D (38) the third, F# (42) the third of the middle one.
    let bd = &c2["rack:BD"];
    assert_eq!((bd.kind.as_str(), bd.bind.as_deref()), ("tap", Some("@rack:BD")));
    assert_eq!(
        (bd.col, bd.row, bd.col_span, bd.row_span),
        (0.0, 2.0 * TAP_ROWS, Some(TAP_COLS), Some(TAP_ROWS))
    );
    assert_eq!((c2["rack:SN"].col, c2["rack:SN"].row), (2.0 * TAP_COLS, 2.0 * TAP_ROWS));
    assert_eq!((c2["rack:HH"].col, c2["rack:HH"].row), (2.0 * TAP_COLS, TAP_ROWS));
    // B (47) is the last cell of the top row, and it adds a sound on its own note.
    let add = &c2["rack+:47"];
    assert_eq!(
        (add.bind.as_deref(), add.col, add.row),
        (Some("@rack+47"), 3.0 * TAP_COLS, 0.0)
    );
    assert_eq!(tabs[1].widgets.len(), 12);
}

#[test]
fn a_kits_fixed_pads_have_no_add_taps_a_gain_under_each_and_its_level_beside_the_grid() {
    let ui = rack_ui(&kit(), true);
    // One octave is one page: no tab column for it.
    assert!(ui.tabs.is_none());
    let mut keys: Vec<_> = ui.widgets.keys().cloned().collect();
    keys.sort();
    assert_eq!(
        keys,
        [
            "kit:gain:BD",
            "kit:gain:HH",
            "kit:gain:SN",
            "kit:volume",
            "rack:BD",
            "rack:HH",
            "rack:SN"
        ]
    );
    assert_eq!(ui.rows, PADS_UP * (TAP_ROWS + 1.0));
    let volume = &ui.widgets["kit:volume"];
    assert_eq!(
        (volume.kind.as_str(), volume.bind.as_deref(), volume.min, volume.max),
        ("knob", Some("Volume"), Some(-40.0), Some(12.0))
    );
    assert_eq!((volume.col, volume.row), (PADS_ACROSS * TAP_COLS, 0.0));
    let gain = &ui.widgets["kit:gain:HH"];
    assert_eq!((gain.kind.as_str(), gain.bind.as_deref()), ("number", Some("Key 42")));
    assert_eq!(
        (gain.min, gain.max, gain.step, gain.row_span),
        (Some(-24.0), Some(12.0), Some(1.0), Some(1.0))
    );
    assert_eq!((gain.col, gain.row), (2.0 * TAP_COLS, TAP_ROWS + 1.0 + TAP_ROWS));
}

#[test]
fn a_kit_over_two_octaves_has_a_tab_each_and_its_level_on_every_tab() {
    let mut sounds = kit();
    sounds.push(RackSound {
        symbol: "CR".into(),
        note: 49.0,
    });
    let ui = rack_ui(&sounds, true);
    let tabs = ui.tabs.as_ref().expect("two octaves");
    assert_eq!(tabs.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["C2", "C3"]);
    assert_eq!(ui.widgets.keys().collect::<Vec<_>>(), ["kit:volume"]);
    assert_eq!(tabs[1].widgets["rack:CR"].col, TAP_COLS);
}

#[test]
fn an_empty_rack_opens_on_the_octave_its_first_tap_lands_in() {
    let ui = rack_ui(&[], false);
    assert!(ui.tabs.is_none());
    assert_eq!(ui.widgets.len(), 12);
    assert_eq!(ui.widgets["rack+:36"].bind.as_deref(), Some("@rack+36"));
}

#[test]
fn a_pad_grid_is_never_smaller_than_2x2_square_ish_and_never_wider_than_four() {
    for n in [0, 1, 4] {
        assert_eq!(rack_grid(n), (2.0, 2.0));
    }
    assert_eq!(rack_grid(8), (3.0, 3.0));
    assert_eq!(rack_grid(9), (3.0, 3.0));
    assert_eq!(rack_grid(16), (4.0, 4.0));
    assert_eq!(rack_grid(24), (4.0, 6.0));
}
