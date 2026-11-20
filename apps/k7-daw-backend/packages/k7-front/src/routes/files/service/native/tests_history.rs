//! tests_history.rs — the history routes through their hook over a real account (`docs/files/history.md`): a page, a
//! checkpoint, a diff and a restore answer the contract's shapes, and the refusals carry their words.

use axum::extract::Request;
use http_body_util::BodyExt;
use serde_json::{Value, json};

use super::history::{Kind, hook};
use super::tests_device::{Dev, device, parse, req};

async fn ask(d: &Dev, kind: Kind, req: Request, owner: bool) -> (u16, Value) {
    let res = hook(
        Some(&d.acct),
        "ann@example.org",
        &d.home,
        kind,
        || owner,
        req,
    )
    .await;
    let status = res.status().as_u16();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, parse(core::str::from_utf8(&bytes).unwrap()))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_four_routes_answer_the_contract_and_refuse_with_a_word() {
    let d = device("history").await;
    let root = format!("{}/workspace/band", d.home.studio);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(format!("{root}/notes.md"), "one\n").unwrap();
    let (s, page) = ask(
        &d,
        Kind::Page,
        req("GET", "/history/page?workspace=band", None),
        true,
    )
    .await;
    assert_eq!((s, page["source"].as_str()), (200, Some("device")));
    assert_eq!(
        (page["head"].as_f64(), page["checkpoints"].is_array()),
        (Some(1.0), true)
    );
    let entry = &page["entries"][0];
    assert_eq!(entry["files"], json!(["notes.md"]));
    assert!(entry["ts"].is_number() && entry["label"].is_string() && entry["parked"].is_number());
    let named = json!({ "workspace": "band", "name": "first" });
    let (s, cp) = ask(
        &d,
        Kind::Checkpoint,
        req("POST", "/history/checkpoint", Some(named)),
        true,
    )
    .await;
    assert_eq!(
        (s, cp["seq"].as_f64(), cp["by"].as_str()),
        (200, Some(1.0), Some("ann@example.org"))
    );
    let at = d.acct.attached_at(&root).unwrap();
    std::fs::write(format!("{root}/notes.md"), "two\n").unwrap();
    at.handle.capture(vec!["notes.md".into()]).await;
    for _ in 0..200 {
        let (_, p) = ask(
            &d,
            Kind::Page,
            req("GET", "/history/page?workspace=band&limit=1", None),
            true,
        )
        .await;
        if p["head"].as_f64() == Some(2.0) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let (s, diff) = ask(
        &d,
        Kind::Diff,
        req("GET", "/history/diff?workspace=band&from=1&to=2", None),
        true,
    )
    .await;
    assert_eq!(
        (
            s,
            diff["files"][0]["before"].as_str(),
            diff["files"][0]["after"].as_str()
        ),
        (200, Some("one\n"), Some("two\n"))
    );
    let (s, back) = ask(
        &d,
        Kind::Restore,
        req(
            "POST",
            "/history/restore",
            Some(json!({ "workspace": "band", "seq": 1 })),
        ),
        true,
    )
    .await;
    assert_eq!((s, back["entry"]["seq"].as_f64()), (200, Some(3.0)));
    assert_eq!(
        std::fs::read_to_string(format!("{root}/notes.md")).unwrap(),
        "one\n"
    );

    let word = |(s, b): (u16, Value)| (s, b["code"].as_str().unwrap_or("").to_string());
    let unknown = req("GET", "/history/page?workspace=nope", None);
    assert_eq!(
        word(ask(&d, Kind::Page, unknown, true).await),
        (404, "unknown-workspace".into())
    );
    let none = req(
        "POST",
        "/history/checkpoint",
        Some(json!({ "workspace": "band" })),
    );
    assert_eq!(
        word(ask(&d, Kind::Checkpoint, none, true).await),
        (400, "bad-request".into())
    );
    let past = req(
        "POST",
        "/history/restore",
        Some(json!({ "workspace": "band", "seq": 99 })),
    );
    assert_eq!(
        word(ask(&d, Kind::Restore, past, true).await),
        (400, "bad-request".into())
    );
    let theirs = req("GET", "/history/page?workspace=band", None);
    assert_eq!(
        word(ask(&d, Kind::Page, theirs, false).await),
        (403, "forbidden".into())
    );
    let missing = req("GET", "/history/page", None);
    assert_eq!(
        word(ask(&d, Kind::Page, missing, true).await),
        (400, "bad-request".into())
    );
}
