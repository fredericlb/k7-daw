//! tests_waiters.rs — the three plugin queries over a fake engine: matched on the id or the path asked, silence read per query, no waiter left behind (2340, 2341).
//! What a reader gets wrong: `shrink` cuts every wait of the link, so only the tests of silence use it.

use std::sync::Arc;

use serde_json::{Value, json};

use super::queries::{self, Reply};
use super::waiters::{Waiters, Want};
use crate::routes::files::service::native::engine::fake::{Rig, until};

async fn params(rig: &Rig, id: &str) -> Reply {
    queries::params(&rig.link, "VST3", id).await
}

fn waiters(rig: &Rig) -> &Waiters {
    &rig.link.0.waiters
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parameters_are_matched_on_the_id_beside_another_surfaces_answer() {
    let rig = Rig::new("wt-params").await;
    rig.engine.queries(|s| {
        s.decoy = true;
        s.params = vec![json!({ "name": "gain" })];
    });
    let (status, body) = params(&rig, "id-1").await;
    assert_eq!(status, 200);
    assert_eq!(
        body,
        json!({ "found": true, "params": [{ "name": "gain" }] })
    );
    // The question went out on the bare session.
    let asked = rig.engine.of(rig.link.session(None).name());
    let frame = asked
        .iter()
        .find(|f| f["type"] == "list-plugin-params")
        .unwrap();
    assert_eq!(
        (frame["format"].as_str(), frame["id"].as_str()),
        (Some("VST3"), Some("id-1"))
    );
    assert_eq!(waiters(&rig).len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_plugin_the_machine_could_not_load_answers_found_false() {
    let rig = Rig::new("wt-notfound").await;
    rig.engine.queries(|s| s.found = false);
    assert_eq!(
        params(&rig, "x").await,
        (200, json!({ "found": false, "params": [] }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_silent_engine_is_a_503_for_parameters_and_a_base_and_a_504_for_the_window() {
    let rig = Rig::new("wt-silent").await;
    rig.engine.queries(|s| s.quiet = true);
    waiters(&rig).shrink(150);
    assert_eq!(params(&rig, "x").await.0, 503);
    let window = queries::window(&rig.link, "VST3", "x").await;
    assert_eq!(window.0, 504);
    assert!(
        window.1["error"]
            .as_str()
            .unwrap()
            .starts_with("the engine never said it opened one")
    );
    assert_eq!(
        queries::base(&rig.link, "VST3", "x", "/b/a.vstpreset", "base/a.vstpreset")
            .await
            .0,
        503
    );
    assert_eq!(waiters(&rig).len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_engine_bound_answers_at_once_and_leaves_no_waiter() {
    let rig = Rig::new("wt-unbound").await;
    rig.link.bind(&json!({}));
    assert_eq!(params(&rig, "x").await.0, 503);
    assert_eq!(queries::window(&rig.link, "VST3", "x").await.0, 504);
    assert_eq!(waiters(&rig).len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_window_that_did_not_open_is_a_502_and_a_base_not_written_a_409() {
    let rig = Rig::new("wt-refused").await;
    rig.engine.queries(|s| {
        s.opened = false;
        s.saved = false;
    });
    let (status, body) = queries::window(&rig.link, "VST3", "gtr").await;
    assert_eq!(status, 502);
    assert_eq!(
        body["error"],
        "the engine could not open a window for gtr — see its log"
    );
    let (status, body) = queries::base(
        &rig.link,
        "VST3",
        "gtr",
        "/b/a.vstpreset",
        "base/a.vstpreset",
    )
    .await;
    assert_eq!(status, 409);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("nothing to snapshot")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_base_is_matched_on_its_path_and_the_engine_writes_it() {
    let rig = Rig::new("wt-base").await;
    rig.engine.queries(|s| s.decoy = true);
    let abs = rig.dir.join("a.vstpreset").display().to_string();
    let got = queries::base(&rig.link, "VST3", "gtr", &abs, "base/a.vstpreset").await;
    assert_eq!(got, (200, json!({ "path": "base/a.vstpreset" })));
    assert_eq!(std::fs::read(&abs).unwrap(), b"VST3");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_burst_of_asks_at_once_is_each_answered_by_its_own_id() {
    let rig = Arc::new(Rig::new("wt-burst").await);
    rig.engine.queries(|s| s.decoy = true);
    let asks: Vec<_> = (0..25)
        .map(|n| {
            let rig = rig.clone();
            tokio::spawn(async move { params(&rig, &format!("p{n}")).await })
        })
        .collect();
    for a in asks {
        assert_eq!(a.await.unwrap().0, 200);
    }
    assert_eq!(waiters(&rig).len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_request_given_up_leaves_no_waiter() {
    let rig = Arc::new(Rig::new("wt-gone").await);
    rig.engine.queries(|s| s.quiet = true);
    let r2 = rig.clone();
    let ask = tokio::spawn(async move { params(&r2, "x").await });
    until(10, "the waiter to register", || waiters(&rig).len() == 1).await;
    ask.abort();
    until(10, "the waiter to leave", || waiters(&rig).len() == 0).await;
}

#[tokio::test]
async fn one_frame_settles_every_waiter_on_its_key_and_no_other() {
    let w = Arc::new(Waiters::default());
    let (a, b) = (w.wait(Want::Params, "id"), w.wait(Want::Params, "id"));
    let other_id = w.wait(Want::Params, "else");
    let other_kind = w.wait(Want::Window, "id");
    w.settle(
        "plugin-param-list",
        &json!({ "type": "plugin-param-list", "id": "id" }),
    );
    for heard in [a.answer().await, b.answer().await] {
        assert_eq!(heard.unwrap()["id"], "id");
    }
    assert_eq!(w.len(), 2);
    drop((other_id, other_kind));
    assert_eq!(w.len(), 0);
}

#[tokio::test]
async fn a_frame_with_no_key_matches_the_empty_one_as_the_clients_read_it() {
    let w = Arc::new(Waiters::default());
    let empty = w.wait(Want::Window, "");
    w.settle(
        "native-plugin-ui-opened",
        &json!({ "type": "native-plugin-ui-opened", "ok": true }),
    );
    assert_eq!(
        empty.answer().await.map(|f| f["ok"].clone()),
        Some(Value::Bool(true))
    );
}

#[tokio::test]
async fn an_answer_settled_inside_the_send_still_finds_its_waiter() {
    let waiters = Arc::new(Waiters::default());
    waiters.shrink(300);
    let frame = json!({ "id": "id-1", "params": [] });
    // A link whose send settles at once: the waiter is already there only if it was registered first.
    let heard = queries::ask_with(&waiters, Want::Params, "id-1", || {
        waiters.settle("plugin-param-list", &frame);
        true
    })
    .await;
    assert_eq!(heard, Some(frame));
    assert_eq!(waiters.len(), 0);
}
