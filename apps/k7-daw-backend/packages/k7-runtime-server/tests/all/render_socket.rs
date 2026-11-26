//! render_socket.rs — `render` over a real socket and the fake engine: the request shape the supervisor sends and
//! the replies it waits for, one render at a time, every refusal, and the doc the engine is handed.

use crate::harness::{Server, Wire, eventually};
use k7_runtime_server::engine::fake::render::{self, Script};
use k7_runtime_server::server::ServeConfig;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;

fn start() -> Server {
    Server::start(ServeConfig::new(0))
}

/// The supervisor's request (`ServedRender.cpp`): the doc, the folder, the stems flag and an id it waits for.
fn request(out: &str, extra: Value) -> String {
    let mut msg = json!({
        "type": "render",
        "id": "job-1",
        "doc": {"name": "m8", "tracks": [{"id": "t", "clips": [{"id": "c", "source": "a.wav"}]}]},
        "outDir": out,
        "stems": true,
    });
    if let (Value::Object(m), Value::Object(e)) = (&mut msg, extra) {
        m.extend(e);
    }
    msg.to_string()
}

/// The next `rendered`, past the engine warnings a parse says on the way.
async fn rendered(w: &mut Wire) -> String {
    loop {
        let t = w.text().await;
        if !t.contains(r#""type":"engine-warning""#) {
            return t;
        }
    }
}

async fn ask(w: &mut Wire, text: &str) -> String {
    w.send(text).await;
    rendered(w).await
}

fn refused(why: &str) -> String {
    format!(r#"{{"error":"{why}","id":"job-1","ok":false,"type":"rendered"}}"#)
}

#[tokio::test]
async fn a_render_answers_the_manifest_under_the_id_and_hands_the_engine_a_validated_doc() {
    let s = start();
    let out = "/m8/render/ok";
    render::script(
        out,
        Script {
            manifest: r#"{"mix":"mix.wav","stems":[{"name":"t","file":"t.wav"}]}"#.into(),
            ..Script::default()
        },
    );
    let mut w = s.connect().await;
    assert_eq!(
        w.ask(&request(out, json!({}))).await,
        r#"{"id":"job-1","manifest":{"mix":"mix.wav","stems":[{"file":"t.wav","name":"t"}]},"ok":true,"type":"rendered"}"#
    );
    let asked = render::asked(out);
    assert_eq!(asked.len(), 1);
    assert!(asked[0].stems);
    let doc: Value = serde_json::from_str(&asked[0].doc_json).unwrap();
    assert_eq!(doc["name"], "m8");
    assert_eq!(doc["sampleRate"], 48000);
    assert_eq!(doc["tracks"][0]["clips"][0]["source"], "a.wav");
}

#[tokio::test]
async fn a_relative_source_is_joined_to_the_base_before_the_roots_test_it() {
    let s = start();
    let out = "/m8/render/base";
    let mut w = s.connect().await;
    w.ask(&request(out, json!({"base": "/songs/x/./sub/.."})))
        .await;
    let doc: Value = serde_json::from_str(&render::asked(out)[0].doc_json).unwrap();
    assert_eq!(doc["tracks"][0]["clips"][0]["source"], "/songs/x/a.wav");
}

#[tokio::test]
async fn a_failed_bounce_is_a_not_ok_reply_with_its_reason() {
    let s = start();
    let out = "/m8/render/fail";
    render::script(
        out,
        Script {
            code: 2,
            error: "render failed: no audio".into(),
            ..Script::default()
        },
    );
    let mut w = s.connect().await;
    assert_eq!(
        w.ask(&request(out, json!({}))).await,
        r#"{"error":"render failed: no audio","id":"job-1","ok":false,"type":"rendered"}"#
    );
}

#[tokio::test]
async fn a_second_render_is_busy_while_one_runs_and_the_slot_frees_after() {
    let s = start();
    let out = "/m8/render/held";
    render::script(
        out,
        Script {
            hold: true,
            manifest: "{}".into(),
            ..Script::default()
        },
    );
    let mut w = s.connect().await;
    w.send(&request(out, json!({}))).await;
    eventually("the bounce began", || render::asked(out).len() == 1).await;
    assert_eq!(
        w.ask(&request("/m8/render/other", json!({}))).await,
        r#"{"busy":true,"error":"a render is already running on this engine","id":"job-1","ok":false,"type":"rendered"}"#
    );
    render::release(out);
    assert_eq!(
        w.text().await,
        r#"{"id":"job-1","manifest":{},"ok":true,"type":"rendered"}"#
    );
    eventually("the slot freed", || {
        !s.ctx.machine.rendering.load(Ordering::SeqCst)
    })
    .await;
    let after = w.ask(&request("/m8/render/after", json!({}))).await;
    assert!(after.contains(r#""ok":true"#), "{after}");
}

#[tokio::test]
async fn every_refusal_is_a_not_ok_reply_and_no_bounce_starts() {
    let s = start();
    let mut w = s.connect().await;
    let need = refused("render: a doc and an outDir are required");
    assert_eq!(
        ask(
            &mut w,
            r#"{"type":"render","id":"job-1","outDir":"/m8/refuse/a"}"#
        )
        .await,
        need
    );
    assert_eq!(
        ask(&mut w, r#"{"type":"render","id":"job-1","doc":{}}"#).await,
        need
    );
    assert_eq!(
        ask(&mut w, &request("//evil/share", json!({}))).await,
        refused("render: '//evil/share' is outside the folders this engine writes")
    );
    assert_eq!(
        ask(
            &mut w,
            &request("/m8/refuse/b", json!({"base": "//evil/share"}))
        )
        .await,
        refused("render: '//evil/share' is outside the folders this engine reads")
    );
    let doc = r#"{"tracks":[{"id":"t","clips":[{"id":"c","source":"//evil/a.wav"}]}]}"#;
    let reply = w
        .ask(&format!(
            r#"{{"type":"render","id":"job-1","outDir":"/m8/refuse/c","doc":{doc}}}"#
        ))
        .await;
    assert_eq!(
        reply,
        refused("render: '//evil/a.wav' is outside the folders this engine may open")
    );
    let reply = w
        .ask(r#"{"type":"render","id":"job-1","outDir":"/m8/refuse/d","doc":{"tempo":"fast"}}"#)
        .await;
    assert!(
        reply.contains(r#""error":"bad project: [json.exception.type_error.302]"#),
        "{reply}"
    );
    for dir in ["a", "b", "c", "d"] {
        assert!(render::asked(&format!("/m8/refuse/{dir}")).is_empty());
    }
}

#[tokio::test]
async fn a_request_with_no_id_is_answered_with_a_null_one() {
    let s = start();
    let mut w = s.connect().await;
    assert_eq!(
        w.ask(r#"{"type":"render"}"#).await,
        r#"{"error":"render: a doc and an outDir are required","id":null,"ok":false,"type":"rendered"}"#
    );
}
