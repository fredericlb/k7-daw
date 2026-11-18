//! The download route over a real account and a served model (1p-D16): the chain found natively, the TypeScript's sentences, the jobs.
//! What a reader gets wrong: a ref that is no string is refused, not guessed at.

use std::sync::atomic::Ordering;

use axum::body::Body;
use axum::extract::Request;
use serde_json::{Value, json};

use super::super::media::testkit::{Reply, serve};
use super::super::media::{Config, Media};
use super::super::tests::{Lines, ctx, scratch};
use super::super::tests_account::start;
use super::super::{Ctx, Tag};
use super::counts;
use super::download::{answer, download_ctx, jobs_of};
use super::tests::{body_of, fixture, with_docs};
use crate::routes::files::service::body;

fn vox(url: &str) -> String {
    format!(
        "cold(true)\nplugin({{ type: \"offline\", name: \"vox\" }})\nparams({{ onnx: \"vox.onnx\" }})\nmodel_url(\"{url}\")\n"
    )
}

const NOURL: &str =
    "cold(true)\nplugin({ type: \"offline\", name: \"bare\" })\nparams({ onnx: \"bare.onnx\" })\n";

fn post(body: &str) -> Request {
    Request::builder()
        .method("POST")
        .uri("/account/cold-chains/download")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn ask(c: &Ctx, media: &Media, body: &str, owner: bool) -> (u16, Value) {
    let gate = move || owner;
    body_of(answer(c.clone(), media, &gate, post(body)).await).await
}

#[test]
fn the_chain_is_found_by_ref_and_the_refusals_are_the_typescripts() {
    let view = json!({ "chains": [
        { "ref": "a", "downloads": [{ "file": "x.onnx", "url": "u1" }, { "file": "x.onnx", "url": "u1" }] },
        { "ref": "b", "downloads": [] },
    ] });
    let both = vec![("x.onnx".to_owned(), "u1".to_owned()); 2];
    assert_eq!(jobs_of(&view, "a"), Ok(both));
    assert_eq!(
        jobs_of(&view, "b"),
        Err((400, "'b' needs no model this machine can download".into()))
    );
    assert_eq!(
        jobs_of(&view, ""),
        Err((404, "'' is not a cold chain of the library".into()))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_download_starts_each_model_in_the_registry_in_the_chains_order() {
    // The server never answers, so the first job is still running whatever the machine's speed.
    let (addr, seen) = serve(|_| Reply::Hang).await;
    let dir = scratch("chains-download");
    let module = "libraries/k7-core/1/modules/cold/vox/module.k7x";
    let twice = "steps([id(\"a\").extends(\"@k7-core/cold/vox\"), id(\"b\").extends(\"@k7-core/cold/vox\"), to([chain(\"@out\")])])\n";
    fixture(
        &dir,
        true,
        &[
            (module, &vox(&format!("{addr}/vox.onnx"))),
            ("libraries/k7-core/1/modules/cold/bare/module.k7x", NOURL),
            ("libraries/k7-core/1/chains/twice.k7x", twice),
            (
                "libraries/k7-core/1/chains/bare.k7x",
                "steps([id(\"s\").extends(\"@k7-core/cold/bare\"), to([chain(\"@out\")])])\n",
            ),
        ],
    );
    let acct = start(&dir);
    let lines = Lines::default();
    let c = with_docs(ctx(&dir, Tag::ColdChainsDownload, "", &lines), &acct, &dir);
    let env = [(
        "STUDIO_HOME".to_owned(),
        crate::routes::files::service::native::tests::slashed(&dir.join("home")),
    )]
    .into();
    let media = Media::new(Config::of(&env, &dir), std::sync::Arc::new(|| ()));
    let (status, got) = ask(&c, &media, r#"{"ref":"@k7-core/stems"}"#, true).await;
    assert_eq!(status, 200, "{got}");
    let jobs = got["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 1);
    let id = jobs[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(media.downloads.job(&id).unwrap().to_json()["id"], id);
    // A file named twice starts twice and, while the first runs, the registry answers one job.
    let (_, got) = ask(&c, &media, r#"{"ref":" @k7-core/twice "}"#, true).await;
    let ids: Vec<&Value> = got["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|j| &j["id"])
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(ids[0], ids[1]);
    assert_eq!(ids[0], &Value::String(id));
    let sentences = [
        (
            r#"{"ref":"nope"}"#,
            404,
            "'nope' is not a cold chain of the library",
        ),
        (r#"{}"#, 404, "'' is not a cold chain of the library"),
        (
            r#"{"ref":"@k7-core/bare"}"#,
            400,
            "'@k7-core/bare' needs no model this machine can download",
        ),
    ];
    for (body, code, error) in sentences {
        assert_eq!(
            ask(&c, &media, body, true).await,
            (code, json!({ "error": error })),
            "{body}"
        );
    }
    assert_eq!(
        ask(&c, &media, "{", true).await.0,
        400,
        "a body that does not parse is node's 400"
    );
    assert_eq!(
        ask(&c, &media, "{", false).await.0,
        400,
        "the body is parsed before the owner is checked"
    );
    let big = format!(r#"{{"ref":"{}"}}"#, "a".repeat(body::JSON_LIMIT));
    assert_eq!(
        ask(&c, &media, &big, true).await,
        (413, json!({ "error": "request entity too large" })),
        "a body over the limit is node's 413"
    );
    let (status, got) = ask(&c, &media, r#"{"ref":"@k7-core/stems"}"#, false).await;
    assert_eq!(
        (status, got["error"].as_str()),
        (403, Some("these workspaces belong to another account"))
    );
    // A ref that is no string is refused with the body's sentence, and counted.
    let before = counts().declines.load(Ordering::Relaxed);
    let r = super::super::refuse::scoped(answer(c.clone(), &media, &|| true, post(r#"{"ref":3}"#)))
        .await;
    let (status, got) = body_of(r).await;
    assert_eq!(
        (status, got["declined"].as_str()),
        (400, Some("body")),
        "{got}"
    );
    assert!(counts().declines.load(Ordering::Relaxed) > before);
    // One fetch for the file, whatever the asks.
    for _ in 0..100 {
        if !seen.lock().unwrap().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(seen.lock().unwrap().len(), 1);
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_download_declines_without_the_account_s_media() {
    let dir = scratch("chains-download-decline");
    let lines = Lines::default();
    let c = ctx(&dir, Tag::ColdChainsDownload, "", &lines);
    let before = counts().declines.load(Ordering::Relaxed);
    assert!(download_ctx(c.clone(), false).is_none());
    assert!(counts().declines.load(Ordering::Relaxed) > before);
    assert!(
        lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("declined") && l.contains("media")),
        "{:?}",
        lines.lock().unwrap()
    );
    assert!(download_ctx(c, true).is_some());
    let _ = std::fs::remove_dir_all(dir);
}
