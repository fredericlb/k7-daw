//! pairing_gate.rs — what the gate lets through: the machine, the open LAN, a paired phone, and an
//! unpaired one, driven through the gate's own interface with no socket.

use super::pairing_store::temp_store;
use k7_runtime_server::pairing::gate::{Gate, GateConfig, Out};
use k7_runtime_server::pairing::store::Pairing;
use serde_json::{Value, json};

const LOCAL: &str = "127.0.0.1";
const LAN: &str = "192.168.1.20";
const PUBLIC: &str = "8.8.8.8";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef";
const MACHINE: u64 = 1;
const PHONE: u64 = 2;
const OTHER: u64 = 3;

fn gate() -> Gate {
    let cfg = GateConfig {
        engine_id: "eng".into(),
        engine_token: TOKEN.into(),
        boot_id: "boot".into(),
        allowed_origins: vec!["https://studio.example.org".into()],
    };
    Gate::new(Pairing::open(temp_store("gate")), cfg)
}

fn say(g: &Gate, id: u64, ip: &str, msg: Value) -> Option<Vec<Out>> {
    let ty = msg["type"].as_str().unwrap_or("").to_string();
    g.screen_message(id, ip, &msg, &ty)
}

fn reply(out: Option<Vec<Out>>) -> Value {
    match out.expect("answered").into_iter().next() {
        Some(Out::Reply(v)) => v,
        other => panic!("not a reply: {other:?}"),
    }
}

fn code(v: &Value) -> &str {
    v["code"].as_str().unwrap_or("")
}

/// A phone asks, the machine approves: the phone's key and the phone's id.
fn pair_phone(g: &Gate, id: u64) -> (String, String) {
    let pending = reply(say(
        g,
        id,
        LAN,
        json!({"type": "pair-request", "name": "iPhone"}),
    ));
    assert_eq!(pending["type"], "pair-pending");
    let out = say(
        g,
        MACHINE,
        LOCAL,
        json!({"type": "pair-approve", "requestId": pending["requestId"]}),
    )
    .unwrap();
    let Out::To(to, paired) = &out[0] else {
        panic!("{out:?}")
    };
    assert_eq!(*to, id);
    (
        paired["key"].as_str().unwrap().to_string(),
        paired["deviceId"].as_str().unwrap().to_string(),
    )
}

#[test]
fn the_machine_drives_and_other_verbs_pass_on() {
    let g = gate();
    assert!(say(&g, MACHINE, LOCAL, json!({"type": "project"})).is_none());
    assert!(g.is_trusted(MACHINE));
    assert_eq!(g.locals(), vec![MACHINE]);
    assert_eq!(
        code(&reply(say(
            &g,
            MACHINE,
            LOCAL,
            json!({"type": "pair-request"})
        ))),
        "not-needed"
    );
}

#[test]
fn an_unpaired_lan_peer_may_probe_ask_and_pair_and_nothing_else() {
    let g = gate();
    let who = reply(say(&g, PHONE, LAN, json!({"type": "whoami"})));
    assert_eq!(
        who,
        json!({"type": "whoami", "id": "eng", "boot": "boot", "needsPairing": true})
    );
    assert!(who.get("token").is_none());
    assert_eq!(
        reply(say(&g, PHONE, LAN, json!({"type": "hello"})))["type"],
        "whoami"
    );
    assert_eq!(
        code(&reply(say(&g, PHONE, LAN, json!({"type": "project"})))),
        "unpaired"
    );
    assert_eq!(
        code(&reply(say(
            &g,
            PHONE,
            LAN,
            json!({"type": "pair-approve", "requestId": "x"})
        ))),
        "unpaired"
    );
    assert_eq!(
        code(&reply(say(
            &g,
            PHONE,
            LAN,
            json!({"type": "pair", "key": "nope"})
        ))),
        "unpaired"
    );
    assert_eq!(
        code(&reply(say(&g, PHONE, LAN, json!({"type": "pair"})))),
        "unpaired"
    );
    assert!(!g.is_trusted(PHONE));
}

#[test]
fn a_request_is_shown_to_the_machine_and_approving_gives_the_phone_its_key_once() {
    let g = gate();
    let out = say(
        &g,
        PHONE,
        LAN,
        json!({"type": "pair-request", "name": "iPhone"}),
    )
    .unwrap();
    let Out::Reply(pending) = &out[0] else {
        panic!()
    };
    assert_eq!(pending["code"].as_str().unwrap().len(), 6);
    let Out::ToLocals(list) = &out[1] else {
        panic!()
    };
    assert_eq!(list["requests"][0]["name"], "iPhone");
    assert_eq!(list["requests"][0]["ip"], LAN);
    assert_eq!(list["requests"][0]["code"], pending["code"]);

    // The musician's own client sees it, and answers.
    let approve = json!({"type": "pair-approve", "requestId": pending["requestId"]});
    let out = say(&g, MACHINE, LOCAL, approve.clone()).unwrap();
    assert!(
        matches!(&out[0], Out::To(PHONE, v) if v["type"] == "paired" && v["key"].as_str().unwrap().len() == 64)
    );
    assert!(matches!(&out[1], Out::ToLocals(v) if v["requests"] == json!([])));
    assert!(matches!(&out[2], Out::ToLocals(v) if v["devices"][0]["name"] == "iPhone"));
    assert!(
        g.is_trusted(PHONE),
        "the phone drives as soon as it is approved"
    );
    assert!(!g.locals().contains(&PHONE));
    assert_eq!(code(&reply(say(&g, MACHINE, LOCAL, approve))), "gone");
}

#[test]
fn a_paired_phone_drives_on_a_new_socket_with_its_key_but_manages_nothing() {
    let g = gate();
    let (key, _) = pair_phone(&g, PHONE);
    let paired = reply(say(&g, OTHER, LAN, json!({"type": "pair", "key": key})));
    assert_eq!(
        (paired["type"].as_str(), paired["name"].as_str()),
        (Some("paired"), Some("iPhone"))
    );
    assert!(say(&g, OTHER, LAN, json!({"type": "project"})).is_none());
    for verb in [
        "list-pair-requests",
        "list-paired-devices",
        "pair-approve",
        "pair-deny",
        "unpair-device",
        "lan-open",
    ] {
        assert_eq!(
            code(&reply(say(&g, OTHER, LAN, json!({"type": verb})))),
            "local-only",
            "{verb}"
        );
    }
}

#[test]
fn the_engines_token_pairs_a_socket_without_a_device() {
    let g = gate();
    assert_eq!(
        reply(say(
            &g,
            PHONE,
            PUBLIC,
            json!({"type": "pair", "key": TOKEN})
        )),
        json!({"type": "paired"})
    );
    assert!(say(&g, PHONE, PUBLIC, json!({"type": "project"})).is_none());
}

#[test]
fn a_denied_request_tells_the_phone() {
    let g = gate();
    let pending = reply(say(&g, PHONE, LAN, json!({"type": "pair-request"})));
    let out = say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "pair-deny", "requestId": pending["requestId"]}),
    )
    .unwrap();
    assert!(
        matches!(&out[0], Out::To(PHONE, v) if v["type"] == "pair-denied" && v["requestId"] == pending["requestId"])
    );
    assert!(matches!(&out[1], Out::ToLocals(v) if v["requests"] == json!([])));
    assert!(!g.is_trusted(PHONE));
}

#[test]
fn a_busy_room_is_told_so() {
    let g = gate();
    for i in 0..8 {
        assert_eq!(
            reply(say(&g, 100 + i, LAN, json!({"type": "pair-request"})))["type"],
            "pair-pending"
        );
    }
    assert_eq!(
        code(&reply(say(&g, PHONE, LAN, json!({"type": "pair-request"})))),
        "busy"
    );
}

#[test]
fn unpairing_untrusts_and_closes_the_live_sockets() {
    let g = gate();
    let (key, id) = pair_phone(&g, PHONE);
    say(&g, OTHER, LAN, json!({"type": "pair", "key": key}));
    let out = say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "unpair-device", "deviceId": id}),
    )
    .unwrap();
    assert_eq!(out[..2], [Out::Close(PHONE), Out::Close(OTHER)]);
    assert!(matches!(&out[2], Out::ToLocals(v) if v["devices"] == json!([])));
    assert!(!g.is_trusted(PHONE) && !g.is_trusted(OTHER));
    assert_eq!(
        code(&reply(say(
            &g,
            OTHER,
            LAN,
            json!({"type": "pair", "key": key})
        ))),
        "unpaired"
    );
}

#[test]
fn the_open_lan_admits_private_peers_until_it_is_closed() {
    let g = gate();
    assert!(!g.lan_open(), "closed until the supervisor says otherwise");
    assert!(
        say(
            &g,
            MACHINE,
            LOCAL,
            json!({"type": "lan-open", "open": true})
        )
        .unwrap()
        .is_empty()
    );
    assert!(say(&g, PHONE, LAN, json!({"type": "project"})).is_none());
    assert_eq!(
        code(&reply(say(&g, PHONE, LAN, json!({"type": "pair-request"})))),
        "not-needed"
    );
    assert_eq!(
        code(&reply(say(&g, OTHER, PUBLIC, json!({"type": "project"})))),
        "unpaired"
    );

    let out = say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "lan-open", "open": false}),
    )
    .unwrap();
    assert_eq!(out, vec![Out::Close(PHONE)]);
    assert!(!g.is_trusted(PHONE));
    assert!(g.is_trusted(MACHINE));
}

#[test]
fn a_paired_phone_survives_the_lan_closing() {
    let g = gate();
    let (key, _) = pair_phone(&g, PHONE);
    say(&g, OTHER, LAN, json!({"type": "pair", "key": key}));
    say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "lan-open", "open": true}),
    );
    assert!(say(&g, OTHER, LAN, json!({"type": "project"})).is_none());
    let out = say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "lan-open", "open": false}),
    )
    .unwrap();
    assert!(out.is_empty(), "trusted by its key, not by the open LAN");
    assert!(g.is_trusted(OTHER));
}

#[test]
fn a_page_from_another_site_is_refused_and_silenced() {
    let g = gate();
    assert!(!g.refuse_origin(1, None));
    assert!(!g.refuse_origin(2, Some("http://127.0.0.1:5173")));
    assert!(!g.refuse_origin(3, Some("https://studio.example.org")));
    assert!(g.refuse_origin(4, Some("https://evil.example")));
    assert!(g.is_refused(4) && !g.is_refused(1));
    assert!(g.forget_socket(4).is_empty());
    assert!(!g.is_refused(4));
}

#[test]
fn a_large_frame_from_an_untrusted_socket_is_refused_before_parsing() {
    let g = gate();
    assert!(g.screen_frame_size(PHONE, LAN, 16 * 1024).is_none());
    let refused = g.screen_frame_size(PHONE, LAN, 16 * 1024 + 1).unwrap();
    assert!(matches!(&refused, Out::Reply(v) if v["code"] == "unpaired"));
    assert!(g.screen_frame_size(MACHINE, LOCAL, 10_000_000).is_none());
    pair_phone(&g, PHONE);
    assert!(g.screen_frame_size(PHONE, LAN, 10_000_000).is_none());
}

#[test]
fn a_closed_socket_withdraws_its_request_and_refreshes_the_list() {
    let g = gate();
    say(&g, PHONE, LAN, json!({"type": "pair-request"}));
    let out = g.forget_socket(PHONE);
    assert!(matches!(&out[..], [Out::ToLocals(v)] if v["requests"] == json!([])));
    assert!(g.forget_socket(PHONE).is_empty());
    let list = reply(say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "list-pair-requests"}),
    ));
    assert_eq!(list["requests"], json!([]));
}

#[test]
fn the_machine_lists_the_paired_devices() {
    let g = gate();
    pair_phone(&g, PHONE);
    let list = reply(say(
        &g,
        MACHINE,
        LOCAL,
        json!({"type": "list-paired-devices"}),
    ));
    assert_eq!(list["type"], "paired-devices");
    assert_eq!(list["devices"][0]["name"], "iPhone");
    assert!(
        list["devices"][0].get("key").is_none(),
        "a key is never listed"
    );
}
