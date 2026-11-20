//! A take is refused while the workspace's git merge waits for choices; disarming is never refused.

use serde_json::json;

use super::Member;
use super::tests_account::world;
use crate::routes::files::service::native::engine::held;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn arming_a_take_is_refused_while_a_merge_waits_and_disarming_is_not() {
    let w = world("vh-merge").await;
    let root = w.b.ctx(Member::Engine, false).root;
    std::fs::create_dir_all(format!("{root}/.git")).unwrap();
    std::fs::write(format!("{root}/.git/k7-merge.json"), "{}").unwrap();
    let arm = json!({ "track": "t", "eventBeats": 4.0, "punch": { "from": 1.0 } });
    let (s, a) = w
        .go(Member::Engine, "", json!({ "op": "takeArm", "arm": arm }))
        .await;
    assert_eq!((s, a["code"].as_str()), (423, Some("merge-pending")), "{a}");
    assert!(held(&w.ctx_session().st).take_arm.is_none());
    let (s, a) = w
        .go(
            Member::Engine,
            "",
            json!({ "op": "arm", "track": "t", "armed": true }),
        )
        .await;
    assert_eq!((s, a["code"].as_str()), (423, Some("merge-pending")), "{a}");
    assert!(w.frames("arm").is_empty(), "nothing reached the engine");
    let (s, _) = w
        .go(Member::Engine, "", json!({ "op": "takeArm", "arm": null }))
        .await;
    assert_eq!(s, 200);
    let (s, _) = w
        .go(
            Member::Engine,
            "",
            json!({ "op": "arm", "track": "t", "armed": false }),
        )
        .await;
    assert_ne!(s, 423);
}
