// The Surfaces tab's rows: the filters' fixed order, each failed one a disabled row and never a hidden one.

use super::*;
use serde_json::{Value, json};

const LAN_API: f64 = 1.0;

fn peer(over: Value) -> Value {
    let mut p = json!({"engineId": "e1", "name": "Studio", "host": "10.0.0.2", "reached": true, "trusted": true});
    merge(&mut p, over);
    p
}

fn hello(over: Value) -> Value {
    let mut h = json!({
        "device": {"name": "Studio", "api": LAN_API, "proto": 1},
        "project": {"workspace": "band", "path": "mix.k7", "label": "band / mix.k7"},
        "surfaces": [{"id": "desk", "type": "turntables"}, {"id": "arr", "type": "timeline"}],
        "screens": []
    });
    merge(&mut h, over);
    h
}

fn merge(into: &mut Value, over: Value) {
    if let (Some(a), Value::Object(b)) = (into.as_object_mut(), over) {
        a.extend(b);
    }
}

fn answered(p: Value, h: Value) -> Value {
    json!({"peer": p, "asked": true, "hello": h})
}

fn rows_with(peers: Vec<Value>, drawable: &[&str]) -> Vec<RowGroup> {
    let input: SurfaceRowsArgs = serde_json::from_value(json!({
        "peers": peers, "selfId": "me", "selfName": "This Mac", "drawable": drawable,
        "versions": {"api": LAN_API, "minProto": 1, "proto": 1}
    }))
    .expect("an input");
    surface_groups(&input)
}

fn rows(peers: Vec<Value>) -> Vec<RowGroup> {
    rows_with(peers, &["turntables", "timeline"])
}

#[test]
fn one_group_per_device_with_a_row_per_surface_in_the_projects_order() {
    let [g] = &rows(vec![answered(peer(json!({})), hello(json!({})))])[..] else {
        panic!("one group")
    };
    assert_eq!(g.device, "Studio");
    assert_eq!(g.project.as_deref(), Some("band / mix.k7"));
    assert_eq!(
        g.rows.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
        ["desk", "arr"]
    );
    let target = g.rows[0].target.as_ref().expect("a target");
    assert_eq!((target.engine_id.as_str(), target.workspace.as_str()), ("e1", "band"));
    assert_eq!((target.project.as_str(), target.surface.as_str()), ("mix.k7", "desk"));
}

#[test]
fn this_devices_own_engine_is_dropped() {
    let own = answered(peer(json!({"engineId": "me"})), hello(json!({})));
    assert!(rows(vec![own]).is_empty());
}

#[test]
fn a_device_is_drawn_once_its_hello_answered_and_says_so_when_it_did_not() {
    let waiting = json!({"peer": peer(json!({})), "asked": false, "hello": null});
    assert!(rows(vec![waiting]).is_empty());
    let silent = answered(peer(json!({})), Value::Null);
    assert_eq!(rows(vec![silent])[0].blocked, Some(RowBlocked::NotAnswering));
    let unreached = json!({"peer": peer(json!({"reached": false})), "asked": false, "hello": null});
    assert_eq!(rows(vec![unreached])[0].blocked, Some(RowBlocked::NotAnswering));
}

#[test]
fn trust_fails_before_the_devices_rows_are_read() {
    let g = &rows(vec![answered(peer(json!({"trusted": false})), hello(json!({})))])[0];
    assert_eq!(g.blocked, Some(RowBlocked::NotTrusted));
    assert!(g.rows.is_empty());
}

#[test]
fn every_row_of_an_incompatible_device_is_disabled_naming_the_older_one() {
    let older = hello(json!({"device": {"name": "Studio", "api": LAN_API - 1.0, "proto": 1}}));
    let g = &rows(vec![answered(peer(json!({})), older)])[0];
    assert!(
        g.rows
            .iter()
            .all(|r| matches!(r.blocked, Some(RowBlocked::Update { .. })) && r.target.is_none())
    );
    assert_eq!(
        g.rows[0].blocked,
        Some(RowBlocked::Update {
            device: "Studio".into()
        })
    );
    let newer = hello(json!({"device": {"name": "Studio", "api": LAN_API + 1.0, "proto": 1}}));
    let g = &rows(vec![answered(peer(json!({})), newer)])[0];
    assert_eq!(
        g.rows[0].blocked,
        Some(RowBlocked::Update {
            device: "This Mac".into()
        })
    );
}

#[test]
fn a_type_with_no_drawer_here_is_disabled_after_the_compatibility() {
    let g = &rows_with(vec![answered(peer(json!({})), hello(json!({})))], &["turntables"])[0];
    assert_eq!(g.rows[0].blocked, None);
    assert_eq!(g.rows[1].blocked, Some(RowBlocked::WindowOnly));
    assert!(g.rows[1].target.is_none());
}

#[test]
fn the_other_devices_already_showing_a_surface_are_listed() {
    let h = hello(json!({"screens": [
        {"surface": "desk", "device": "iPad", "since": 1},
        {"surface": "desk", "device": "Pixel", "since": 2}
    ]}));
    let g = &rows(vec![answered(peer(json!({})), h)])[0];
    assert_eq!(g.rows[0].also_on, ["iPad", "Pixel"]);
    assert!(g.rows[1].also_on.is_empty());
}

#[test]
fn a_device_with_no_project_and_one_whose_project_declares_nothing_are_two_empties() {
    let none = hello(json!({"project": null, "surfaces": []}));
    assert_eq!(rows(vec![answered(peer(json!({})), none)])[0].project, None);
    let bare = hello(json!({"surfaces": []}));
    let g = &rows(vec![answered(peer(json!({})), bare)])[0];
    assert_eq!(g.project.as_deref(), Some("band / mix.k7"));
    assert!(g.rows.is_empty());
}

#[test]
fn a_device_is_named_by_its_hello_then_its_peer_then_its_host() {
    let nameless = hello(json!({"device": {"name": "", "api": LAN_API, "proto": 1}}));
    let named = peer(json!({"name": "Peer name"}));
    assert_eq!(rows(vec![answered(named, nameless.clone())])[0].device, "Peer name");
    let bare = peer(json!({"name": ""}));
    assert_eq!(rows(vec![answered(bare, nameless)])[0].device, "10.0.0.2");
}

#[test]
fn nothing_is_older_when_both_speak_the_same() {
    let device = RowDevice {
        name: "Me".into(),
        api: LAN_API,
        proto: 1.0,
    };
    let v = RowsVersions {
        api: LAN_API,
        min_proto: 1.0,
        proto: 1.0,
    };
    assert_eq!(older_device(&device, "Me", &v), None);
    let behind = RowDevice {
        proto: 0.0,
        ..device.clone()
    };
    assert_eq!(older_device(&behind, "Me", &v).as_deref(), Some("Me"));
    let ahead = RowDevice { proto: 2.0, ..device };
    assert_eq!(older_device(&ahead, "This", &v).as_deref(), Some("This"));
}
