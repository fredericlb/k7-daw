// rules.rs — the `.cube` reader, the lens match and the grip drag, with the cases their TypeScript tests held.
// A reader would get wrong that these are the TypeScript tests' inputs unchanged: they were the oracle the Rust answers were checked against.

use eyes::cameras::{Device, match_camera};
use eyes::cube::parse_cube;
use eyes::rect::{Grip, MIN_SIDE, Rect, drag_rect};

// The identity at size 2: eight entries, red varying fastest, then green, then blue.
const IDENTITY: &str = "# made by hand
TITLE \"identity\"
LUT_3D_SIZE 2
0 0 0
1 0 0
0 1 0
1 1 0
0 0 1
1 0 1
0 1 1
1 1 1
";

#[test]
fn the_cube_reads_the_size_and_every_entry_red_fastest_as_rgba() {
    let lut = parse_cube(IDENTITY).unwrap();
    assert_eq!(lut.size, 2);
    assert_eq!(lut.data.len(), 8 * 4);
    assert_eq!(&lut.data[4..8], &[1.0, 0.0, 0.0, 1.0]);
    assert_eq!(&lut.data[8..12], &[0.0, 1.0, 0.0, 1.0]);
}

#[test]
fn the_cube_folds_a_domain_other_than_0_to_1_back_onto_it() {
    let text = IDENTITY.replace("LUT_3D_SIZE 2", "LUT_3D_SIZE 2\nDOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 2");
    assert_eq!(&parse_cube(&text).unwrap().data[4..8], &[0.5, 0.0, 0.0, 1.0]);
}

#[test]
fn the_cube_refuses_a_1d_lut_a_missing_size_and_a_short_file_by_name() {
    assert!(parse_cube("LUT_1D_SIZE 4\n0 0 0\n").unwrap_err().contains("1D LUT"));
    assert!(parse_cube("0 0 0\n").unwrap_err().contains("LUT_3D_SIZE missing"));
    let short = IDENTITY.replace("1 1 1\n", "");
    assert!(parse_cube(&short).unwrap_err().contains("7 entries for a 2³ LUT"));
}

#[test]
fn the_cube_reads_a_windows_file_and_refuses_a_size_out_of_range() {
    assert_eq!(parse_cube(&IDENTITY.replace('\n', "\r\n")).unwrap().size, 2);
    assert!(parse_cube("LUT_3D_SIZE 1\n").unwrap_err().contains("expected 2 to 256"));
    assert!(parse_cube("LUT_3D_SIZE 300\n").unwrap_err().contains("LUT_3D_SIZE 300"));
}

fn cam(id: &str, label: &str) -> Device {
    Device {
        device_id: id.into(),
        kind: "videoinput".into(),
        label: label.into(),
    }
}

fn here() -> Vec<Device> {
    vec![
        cam("a1", "FaceTime HD Camera"),
        cam("b2", "Logitech BRIO (046d:085e)"),
        Device {
            device_id: "m1".into(),
            kind: "audioinput".into(),
            label: "Built-in Microphone".into(),
        },
    ]
}

fn id_of(label: &str, devices: &[Device]) -> Option<String> {
    match_camera(label, devices).device_id
}

#[test]
fn a_label_takes_the_exact_name_first_then_ignores_case_and_stray_spaces() {
    assert_eq!(id_of("FaceTime HD Camera", &here()).as_deref(), Some("a1"));
    assert_eq!(id_of("  facetime hd camera ", &here()).as_deref(), Some("a1"));
}

#[test]
fn a_label_finds_a_camera_the_browser_dressed_up_with_ids_and_the_other_way_round() {
    assert_eq!(id_of("Logitech BRIO", &here()).as_deref(), Some("b2"));
    assert_eq!(id_of("Logitech BRIO (046d:085e)", &here()).as_deref(), Some("b2"));
}

#[test]
fn a_label_never_matches_a_microphone() {
    let m = match_camera("Built-in Microphone", &here());
    assert!(m.problem.unwrap().contains("no camera called"));
}

#[test]
fn a_missing_lens_falls_back_to_the_default_and_names_what_the_machine_has() {
    let m = match_camera("Blackmagic", &here());
    assert_eq!(m.device_id, None);
    let problem = m.problem.unwrap();
    assert!(problem.contains("no camera called \"Blackmagic\""));
    assert!(problem.contains("FaceTime HD Camera"));
    assert!(problem.contains("Logitech BRIO (046d:085e)"));
    assert!(
        match_camera("anything", &[])
            .problem
            .unwrap()
            .contains("no camera at all")
    );
}

#[test]
fn an_empty_label_is_the_default_which_is_not_a_problem() {
    let m = match_camera("", &here());
    assert_eq!((m.device_id, m.problem), (None, None));
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect { x, y, w, h }
}

#[test]
fn a_body_drag_moves_and_changes_no_size_in_whole_pixels() {
    let b = rect(100.0, 100.0, 200.0, 200.0);
    assert_eq!(drag_rect(b, Grip::Body, 10.0, -20.0), rect(110.0, 80.0, 200.0, 200.0));
    assert_eq!(drag_rect(b, Grip::Body, 0.4, 0.6), rect(100.0, 101.0, 200.0, 200.0));
    assert_eq!(drag_rect(b, Grip::Body, 0.0, 0.0), b);
}

#[test]
fn a_resize_pins_the_opposite_edge() {
    let b = rect(100.0, 100.0, 200.0, 200.0);
    let w = drag_rect(b, Grip::W, 50.0, 0.0);
    assert_eq!(w, rect(150.0, 100.0, 150.0, 200.0));
    assert_eq!(w.x + w.w, 300.0);
    assert_eq!(drag_rect(b, Grip::E, 50.0, 0.0), rect(100.0, 100.0, 250.0, 200.0));
    assert_eq!(drag_rect(b, Grip::Se, 40.0, 60.0), rect(100.0, 100.0, 240.0, 260.0));
    assert_eq!(drag_rect(b, Grip::Nw, 40.0, 60.0), rect(140.0, 160.0, 160.0, 140.0));
}

#[test]
fn a_resize_stops_at_the_minimum_side_rather_than_inverting() {
    let b = rect(100.0, 100.0, 200.0, 200.0);
    assert_eq!(drag_rect(b, Grip::E, -1000.0, 0.0).w, MIN_SIDE);
    let west = drag_rect(b, Grip::W, 1000.0, 0.0);
    assert_eq!(west.w, MIN_SIDE);
    assert_eq!(west.x + west.w, 300.0);
}

#[test]
fn a_half_pixel_rounds_toward_plus_infinity_as_the_page_did() {
    let b = rect(0.0, 0.0, 200.0, 200.0);
    assert_eq!(drag_rect(b, Grip::Body, -2.5, 2.5), rect(-2.0, 3.0, 200.0, 200.0));
}

#[test]
fn grip_codes_name_the_page_s_list_in_order() {
    assert_eq!(Grip::from_code(0), Some(Grip::Body));
    assert_eq!(Grip::from_code(8), Some(Grip::Sw));
    assert_eq!(Grip::from_code(9), None);
}
