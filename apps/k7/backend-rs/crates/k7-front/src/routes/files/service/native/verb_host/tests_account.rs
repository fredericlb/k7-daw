//! The account's members behind the frame (3f, M9): the engine's, over the fake engine, then onnx and the library hub.
//! What a reader gets wrong: the engine is the link's fake, so what is judged is the frame sent and the answer read, not an audio device.

use std::time::Duration;

use serde_json::{Value, json};

use super::tests::{Bed, bed};
use super::{Member, account};
use crate::routes::files::service::native::engine::held;
use crate::routes::files::service::native::engine::testkit::Rig;

pub(super) struct World {
    pub(super) b: Bed,
    pub(super) rig: Rig,
}

pub(super) async fn world(name: &str) -> World {
    World {
        b: bed(name).await,
        rig: Rig::new(name).await,
    }
}

impl World {
    pub(super) async fn go(&self, m: Member, query: &str, body: Value) -> (u16, Value) {
        let mut ctx = self.b.ctx(m, false);
        ctx.account = "acct".into();
        ctx.verbs = account::with(None, Some(self.rig.link.clone()));
        self.b.send(m, query, body, Some(ctx)).await
    }

    /// The frames of `kind` the engine has read, after the first arrived.
    async fn seen(&self, kind: &str) -> Vec<Value> {
        crate::routes::files::service::native::engine::testkit::until(5, kind, || {
            !self.frames(kind).is_empty()
        })
        .await;
        self.frames(kind)
    }

    pub(super) fn frames(&self, kind: &str) -> Vec<Value> {
        self.rig
            .engine
            .got()
            .into_iter()
            .map(|g| g.frame)
            .filter(|f| f["type"] == kind)
            .collect()
    }

    /// An account with no engine bound.
    fn unbind(&self) {
        self.rig.link.bind(&json!({}));
    }

    pub(super) fn ctx_session(
        &self,
    ) -> std::sync::Arc<crate::routes::files::service::native::engine::Session> {
        let root = self.b.ctx(Member::Engine, false).root;
        self.rig.link.session(Some(&root))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plugins_are_the_engines_list_filtered_and_empty_with_no_engine() {
    let w = world("vh-plugins").await;
    w.rig
        .engine
        .plugins(vec![json!({ "name": "Synth", "format": "VST3" })]);
    let (s, a) = w.go(Member::Plugins, "filter=VST3", json!({})).await;
    assert_eq!(
        (s, a["plugins"][0]["name"].as_str()),
        (200, Some("Synth")),
        "{a}"
    );
    assert_eq!(w.frames("list-plugins")[0]["filter"], "VST3");
    let (s, a) = w.go(Member::Plugins, "", json!({})).await;
    assert_eq!(s, 200);
    assert!(w.frames("list-plugins")[1].get("filter").is_none(), "{a}");
    let long = format!("filter={}", "x".repeat(10_000));
    assert_eq!(w.go(Member::Plugins, &long, json!({})).await.0, 400);
    w.unbind();
    let (s, a) = w.go(Member::Plugins, "", json!({})).await;
    assert_eq!((s, a), (200, json!({ "plugins": [] })));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_silent_engine_gives_an_empty_list_not_an_error() {
    let w = world("vh-plugins-silent").await;
    w.rig.engine.silent(true);
    w.rig.link.0.waiters.shrink(200);
    let (s, a) = w.go(Member::Plugins, "", json!({})).await;
    assert_eq!((s, a), (200, json!({ "plugins": [] })));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_rescan_forces_on_request_and_two_at_once_send_one_frame() {
    let w = world("vh-rescan").await;
    w.rig.engine.plugins(vec![json!({ "name": "P" })]);
    let (s, a) = w
        .go(Member::PluginsRescan, "", json!({ "force": true }))
        .await;
    assert_eq!(
        (s, a["plugins"].as_array().map(Vec::len)),
        (200, Some(1)),
        "{a}"
    );
    assert_eq!(w.frames("rescan-plugins")[0]["force"], true);
    assert_eq!(
        w.go(Member::PluginsRescan, "", json!({ "force": "yes" }))
            .await
            .0,
        400
    );
    // The scan ends on the engine's status: a list frame (its own, or another ask's) does not end it, and a second rescan joins it.
    let before = w.frames("rescan-plugins").len();
    w.rig.engine.rescan_after(Duration::from_millis(500));
    let start = std::time::Instant::now();
    let timed = |m: Member| {
        let w = &w;
        async move {
            let r = w.go(m, "", json!({})).await;
            (r, start.elapsed())
        }
    };
    let ((a, ta), (b, tb), (c, tc)) = tokio::join!(
        timed(Member::PluginsRescan),
        timed(Member::PluginsRescan),
        async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            timed(Member::Plugins).await
        }
    );
    assert_eq!((a.0, b.0, c.0), (200, 200, 200));
    for r in [&a, &b, &c] {
        assert_eq!(r.1["plugins"].as_array().map(Vec::len), Some(1), "{r:?}");
    }
    assert!(
        ta >= Duration::from_millis(450) && tb >= Duration::from_millis(450),
        "{ta:?} {tb:?}"
    );
    assert!(
        tc < Duration::from_millis(450),
        "the plain list answers at once: {tc:?}"
    );
    assert_eq!(w.frames("rescan-plugins").len(), before + 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn devices_are_listed_and_a_stereo_pair_is_heard_only_by_an_engine() {
    let w = world("vh-devices").await;
    w.rig.engine.devices(vec![json!({ "name": "Built-in" })]);
    let (s, a) = w.go(Member::Devices, "", json!({})).await;
    assert_eq!(
        (s, a["devices"][0]["name"].as_str()),
        (200, Some("Built-in")),
        "{a}"
    );
    let (s, a) = w
        .go(
            Member::DevicesStereo,
            "",
            json!({ "channel": 3, "stereo": true }),
        )
        .await;
    assert_eq!((s, a), (200, json!({ "heard": true })));
    assert_eq!(w.seen("set-input-stereo").await[0]["channel"], 3);
    for bad in [
        json!({ "channel": 0, "stereo": true }),
        json!({ "channel": 1, "stereo": "yes" }),
        json!({ "channel": 99999, "stereo": true }),
        json!({}),
    ] {
        assert_eq!(w.go(Member::DevicesStereo, "", bad).await.0, 400);
    }
    // No engine: nothing heard, nothing listed.
    w.unbind();
    let (_, a) = w
        .go(
            Member::DevicesStereo,
            "",
            json!({ "channel": 1, "stereo": true }),
        )
        .await;
    assert_eq!(a, json!({ "heard": false }));
    let (_, a) = w.go(Member::Devices, "", json!({})).await;
    assert_eq!(a, json!({ "devices": [] }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_engines_ops_send_their_frames_and_arm_waits_for_its_answer() {
    let w = world("vh-engine").await;
    let (s, a) = w
        .go(
            Member::Engine,
            "",
            json!({ "op": "transport", "action": "play" }),
        )
        .await;
    assert_eq!((s, a), (200, json!({ "sent": true })));
    assert_eq!(w.seen("transport").await[0]["action"], "play");
    let (s, a) = w
        .go(
            Member::Engine,
            "",
            json!({ "op": "arm", "track": "t1", "armed": false }),
        )
        .await;
    assert_eq!(
        (s, a["armed"].clone(), a["lines"].clone()),
        (200, json!(false), json!([])),
        "{a}"
    );
    assert_eq!(w.seen("arm").await[0]["armed"], false);
    let (_, a) = w
        .go(Member::Engine, "", json!({ "op": "showUi", "id": "p1" }))
        .await;
    assert_eq!(a["lines"], json!([]));
    assert_eq!(w.seen("show-native-plugin-ui").await[0]["id"], "p1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_arm_with_no_engine_or_a_silent_one_says_why_and_a_refused_one_says_the_engines_sentence()
 {
    let w = world("vh-arm").await;
    w.rig.engine.mute_arm();
    w.rig.link.0.waiters.shrink(200);
    let (_, a) = w
        .go(Member::Engine, "", json!({ "op": "arm", "track": "t" }))
        .await;
    assert_eq!(a["armed"], Value::Null, "{a}");
    assert!(
        a["lines"][0]
            .as_str()
            .unwrap()
            .contains("engine did not answer the arm of \"t\""),
        "{a}"
    );
    let w2 = world("vh-arm-refused").await;
    w2.rig.engine.refuse_arm("no input device for t");
    let (_, a) = w2
        .go(Member::Engine, "", json!({ "op": "arm", "track": "t" }))
        .await;
    assert_eq!(
        (a["armed"].clone(), a["lines"].clone()),
        (Value::Null, json!(["no input device for t"])),
        "{a}"
    );
    w.unbind();
    let (s, a) = w
        .go(Member::Engine, "", json!({ "op": "arm", "track": "t" }))
        .await;
    assert_eq!(s, 200);
    assert_eq!(a["armed"], Value::Null);
    assert!(
        a["lines"][0].as_str().unwrap().contains("engine offline"),
        "{a}"
    );
    let (_, a) = w
        .go(
            Member::Engine,
            "",
            json!({ "op": "transport", "action": "stop" }),
        )
        .await;
    assert_eq!(a, json!({ "sent": false }));
    let (_, a) = w
        .go(Member::Engine, "", json!({ "op": "showUi", "id": "p" }))
        .await;
    assert!(a["lines"][0].as_str().unwrap().contains("engine offline"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_takes_arm_and_a_timelines_audition_are_the_sessions_state() {
    let w = world("vh-state").await;
    let arm = json!({ "track": "t", "eventBeats": 4.0, "punch": { "from": 1.0, "to": 3.0 } });
    let (s, _) = w
        .go(Member::Engine, "", json!({ "op": "takeArm", "arm": arm }))
        .await;
    assert_eq!(s, 200);
    let st = w.ctx_session();
    let held_arm = held(&st.st).take_arm.clone().expect("armed");
    assert_eq!(
        (held_arm.track.as_str(), held_arm.from, held_arm.to),
        ("t", 1.0, Some(3.0))
    );
    w.go(Member::Engine, "", json!({ "op": "takeArm", "arm": null }))
        .await;
    assert!(held(&st.st).take_arm.is_none());
    w.go(
        Member::Engine,
        "",
        json!({ "op": "timelinePlay", "id": "tl1" }),
    )
    .await;
    assert_eq!(held(&st.st).audition.as_deref(), Some("tl1"));
    w.go(
        Member::Engine,
        "",
        json!({ "op": "timelinePlay", "id": null }),
    )
    .await;
    assert!(held(&st.st).audition.is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hostile_engine_bodies_are_400s_and_change_nothing() {
    let w = world("vh-hostile").await;
    let st = w.ctx_session();
    let long = "x".repeat(300);
    for bad in [
        json!({}),
        json!({ "op": "reboot" }),
        json!({ "op": 5 }),
        json!({ "op": "transport", "action": "play now" }),
        json!({ "op": "transport", "action": "../x" }),
        json!({ "op": "transport" }),
        json!({ "op": "arm", "track": "" }),
        json!({ "op": "arm", "track": long }),
        json!({ "op": "arm", "track": "t", "armed": "yes" }),
        json!({ "op": "takeArm", "arm": { "track": "t", "eventBeats": "4", "punch": { "from": 0 } } }),
        json!({ "op": "takeArm", "arm": { "track": "t", "eventBeats": 4, "punch": { "from": null } } }),
        json!({ "op": "takeArm", "arm": { "track": "t", "eventBeats": 4 } }),
        json!({ "op": "takeArm", "arm": 7 }),
        json!({ "op": "timelinePlay", "id": 7 }),
        json!({ "op": "timelinePlay", "id": "" }),
        json!({ "op": "showUi", "id": "" }),
    ] {
        let (s, a) = w.go(Member::Engine, "", bad.clone()).await;
        assert_eq!(s, 400, "{bad}: {a}");
    }
    assert!(held(&st.st).take_arm.is_none() && held(&st.st).audition.is_none());
    assert!(w.frames("transport").is_empty() && w.frames("arm").is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn onnx_is_null_with_no_machine_and_refuses_a_hostile_set() {
    let w = world("vh-onnx").await;
    let (s, a) = w.go(Member::Onnx, "", json!({})).await;
    assert_eq!((s, a), (200, json!({ "providers": null })));
    let many: Vec<String> = (0..1000).map(|i| format!("p{i}")).collect();
    for bad in [
        json!({ "set": many }),
        json!({ "set": "cpu" }),
        json!({ "set": [1] }),
        json!({ "set": [""] }),
        json!({ "set": ["x".repeat(500)] }),
    ] {
        assert_eq!(w.go(Member::Onnx, "", bad.clone()).await.0, 400, "{bad}");
    }
    let (s, _) = w.go(Member::Onnx, "", json!({ "set": ["cpu"] })).await;
    assert_eq!(s, 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn another_member_goes_to_the_jobs_driver_or_is_off() {
    let w = world("vh-delegate").await;
    let (s, a) = w.go(Member::Peak, "", json!({ "file": "/w/a.wav" })).await;
    assert_eq!((s, a["code"].as_str()), (503, Some("off")), "{a}");
    // An account with no engine link at all serves none of the engine's members.
    let mut ctx = w.b.ctx(Member::Plugins, false);
    ctx.verbs = account::with(None, None);
    let (s, a) = w.b.send(Member::Plugins, "", json!({}), Some(ctx)).await;
    assert_eq!((s, a["code"].as_str()), (503, Some("off")), "{a}");
    let _ = Duration::ZERO;
}

/// A machine that answers an `onnx-providers` ask with a script, its reply carrying the ask's id.
struct Machine {
    reply: Option<Value>,
    queued: Vec<Vec<u8>>,
}

impl crate::routes::engine_reach::job::JobChannel for Machine {
    fn send_text(&mut self, s: String) {
        let id = serde_json::from_str::<Value>(&s).unwrap()["id"].clone();
        if let Some(mut r) = self.reply.clone() {
            r["id"] = id;
            self.queued.push(r.to_string().into_bytes());
        }
    }

    async fn recv(&mut self) -> Option<Vec<u8>> {
        self.queued.pop()
    }

    fn close(&mut self) {}
}

async fn onnx_over(reply: Option<Value>) -> (u16, Value) {
    let mut m = Machine {
        reply,
        queued: vec![],
    };
    let body = serde_json::Map::from_iter([("type".to_string(), json!("onnx-providers"))]);
    let r = super::onnx::over(&mut m, body).await;
    let (s, t) = crate::routes::files::service::native::tests_account::text(r).await;
    (s, serde_json::from_str(&t).unwrap())
}

#[tokio::test]
async fn the_machines_onnx_reply_is_its_string_names_and_a_refusal_or_a_lost_machine_is_no_machine()
{
    let named = json!({ "type": "onnx-providers", "providers": ["cuda", 3, null, "cpu"] });
    assert_eq!(
        onnx_over(Some(named)).await,
        (200, json!({ "providers": ["cuda", "cpu"] }))
    );
    let none = json!({ "type": "onnx-providers" });
    assert_eq!(
        onnx_over(Some(none)).await,
        (200, json!({ "providers": [] }))
    );
    let refused = json!({ "type": "error", "error": "no onnx here" });
    let (s, a) = onnx_over(Some(refused)).await;
    assert_eq!(
        (s, a["code"].as_str(), a["error"].as_str()),
        (503, Some("no-machine"), Some("no onnx here"))
    );
    let (s, a) = onnx_over(None).await;
    assert_eq!((s, a["code"].as_str()), (503, Some("no-machine")), "{a}");
}
