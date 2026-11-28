use super::*;

fn ids() -> MainSurfaceIds {
    MainSurfaceIds {
        timeline: Some("arr".into()),
        turntables: Some("desk".into()),
        viewport: Some("frame".into()),
    }
}

fn screen(surface: &str, device: &str, since: f64) -> RemoteScreen {
    RemoteScreen {
        surface: surface.into(),
        device: device.into(),
        since,
    }
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| (*s).to_owned()).collect()
}

#[test]
fn a_remotes_surface_maps_to_the_pane_or_window_it_is_on_this_device() {
    let screens = [
        screen("desk", "iPad", 1.0),
        screen("frame", "Pixel", 2.0),
        screen("arr", "Mac", 3.0),
    ];
    let held = in_use_by(&screens, &ids());
    assert_eq!(held.turntables.as_deref(), Some("iPad"));
    assert_eq!(held.viewport.as_deref(), Some("Pixel"));
    assert_eq!(held.timeline.as_deref(), Some("Mac"));
}

#[test]
fn a_surface_with_no_pane_here_is_never_blocked_mastering_among_them() {
    assert_eq!(in_use_by(&[screen("mix", "iPad", 1.0)], &ids()), InUse::default());
    let none = MainSurfaceIds {
        turntables: None,
        ..ids()
    };
    assert_eq!(in_use_by(&[screen("desk", "iPad", 1.0)], &none), InUse::default());
}

#[test]
fn the_device_that_came_first_is_named_when_two_remotes_show_one_surface() {
    let screens = [screen("desk", "late", 9.0), screen("desk", "early", 2.0)];
    assert_eq!(in_use_by(&screens, &ids()).turntables.as_deref(), Some("early"));
}

const ORDER: [&str; 4] = ["timeline", "turntables", "chains", "editor"];

#[test]
fn the_next_pane_that_is_not_blocked_comes_in_the_switchers_order() {
    let order = strings(&ORDER);
    assert_eq!(
        next_free(&order, "turntables", &strings(&["turntables"])).as_deref(),
        Some("chains")
    );
    assert_eq!(
        next_free(&order, "timeline", &strings(&["timeline", "turntables"])).as_deref(),
        Some("chains")
    );
}

#[test]
fn it_wraps_and_answers_none_when_every_other_pane_is_blocked() {
    let order = strings(&ORDER);
    assert_eq!(
        next_free(&order, "editor", &strings(&["editor", "timeline"])).as_deref(),
        Some("turntables")
    );
    assert_eq!(next_free(&strings(&["a", "b"]), "a", &strings(&["a", "b"])), None);
}

#[test]
fn screens_compare_by_content() {
    let a = [screen("desk", "iPad", 1.0)];
    assert!(same_screens(&a, &[screen("desk", "iPad", 1.0)]));
    assert!(!same_screens(&a, &[]));
}
