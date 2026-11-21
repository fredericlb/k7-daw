//! The declines of a piece are refusals with a sentence, (4-7, D1): one test per reason
//! of `Declined`, the compile failure's body, and no open project answered rather than declined.

use k7_file_service::space::Declined;
use serde_json::json;

use super::Chaos;
use super::Tag;
use super::refuse;
use super::tests::{Lines, ctx, scratch};
use super::tests_account::{call, start, text};
use super::tests_docs::{ROUTES, answering, declined, route, value, world};
use crate::routes::files::service::routes::{self, H};

/// A file read, answering, whose native side declines for `why`: the response.
async fn declined_read(why: Declined) -> (u16, String, bool) {
    let dir = scratch("refuse");
    std::fs::write(dir.join("home/workspace/songs/a.txt"), "one\n").unwrap();
    let lines = Lines::default();
    let mut c = ctx(&dir, Tag::File, "a.txt", &lines);
    c.chaos = Chaos::Decline(why);
    let call = call(&dir, "path=a.txt", json!({}), Some(c));
    let r = refuse::scoped(routes::dispatch(H::FileGet, &call)).await;
    let wait = r.headers().contains_key(hyper::header::RETRY_AFTER);
    let (status, body) = text(r).await;
    super::tests::remove_tree(&dir);
    (status, body, wait)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn every_reason_of_declined_is_a_refusal_with_its_sentence() {
    for why in Declined::ALL {
        let (status, body, wait) = declined_read(why).await;
        let (want, sentence) = refuse::sentence(why.as_str());
        assert_eq!(status, want, "{why:?}: {body}");
        assert_eq!(
            value(&body),
            json!({ "error": sentence, "declined": why.as_str() }),
            "{why:?}"
        );
        assert_eq!(wait, why == Declined::Busy, "{why:?}: Retry-After");
    }
}

#[test]
fn every_reason_of_declined_has_its_own_sentence() {
    let mut said = std::collections::HashSet::new();
    for why in Declined::ALL {
        assert!(refuse::known().any(|k| k == why.as_str()), "{why:?}");
        assert!(said.insert(refuse::sentence(why.as_str()).1), "{why:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_project_that_will_not_compile_is_worded_as_the_compiler_throws_it() {
    let (dir, acct) = world("refuse-failure").await;
    let lines = Lines::default();
    let project = dir.join("home/workspace/songs/p.k7/project.k7x");
    for (what, source) in [
        ("a broken file", "project({ tempo: \n"),
        ("a newer format", "format(999)\nproject({ tempo: 120 })\n"),
    ] {
        std::fs::write(&project, source).unwrap();
        for (h, tag) in [
            (H::ProjectDoc, Tag::ProjectDoc),
            (H::EngineDoc, Tag::EngineDoc),
            (H::Graph, Tag::Graph),
        ] {
            let c = answering(&acct, &dir, tag, &lines);
            let ours = route(&dir, h, "", Some(c)).await;
            assert_eq!(ours.0, 400, "{what} {h:?}: {}", ours.1);
            assert!(value(&ours.1)["error"].is_string(), "{what} {h:?}");
        }
    }
    assert_eq!(declined(&dir), 0, "{:?}", lines.lock().unwrap());
    super::tests::remove_tree(&dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn no_open_project_is_what_the_route_answers_and_never_a_decline() {
    let dir = scratch("refuse-idle");
    let ws = dir.join("home/workspace/songs");
    std::fs::create_dir_all(ws.join("p.k7")).unwrap();
    let acct = start(&dir);
    let lines = Lines::default();
    for (h, tag) in ROUTES {
        let c = answering(&acct, &dir, tag, &lines);
        let ours = route(&dir, h, "", Some(c)).await;
        assert!(
            value(&ours.1).get("declined").is_none(),
            "{h:?}: {}",
            ours.1
        );
    }
    assert_eq!(declined(&dir), 0, "{:?}", lines.lock().unwrap());
    super::tests::remove_tree(&dir);
}
