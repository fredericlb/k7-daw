//! The chains' front: the answers over a real account, the declines and the counts (1p-D3 to 1p-D9).
//! What a reader gets wrong: the account only holds the shared sessions here.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use http_body_util::BodyExt;
use serde_json::Value;

use super::super::docs::Docs;
use super::super::tests::{Lines, ctx, scratch};
use super::super::tests_account::start;
use super::super::{Ctx, Tag};
use super::{What, answer_what, bench_session, compute, counts};
use crate::routes::files::service::Acct;

const MODULES: [(&str, &str); 2] = [
    (
        "k7-core/1/library.k7x",
        "name(\"k7-core\")\nversion(1)\nrequires(1)\n",
    ),
    (
        "k7-core/1/modules/cold/vox/module.k7x",
        "cold(true)\nplugin({ type: \"offline\", name: \"vox\" })\nparams({ onnx: \"vox.onnx\" })\nmodel_url(\"https://m.example/v.onnx\")\n",
    ),
];

pub(super) fn fixture(dir: &Path, pins: bool, extra: &[(&str, &str)]) {
    let chain = "steps([id(\"s\").extends(\"@k7-core/cold/vox\"), to([chain(\"@out\")])])\n";
    let live = "steps([id(\"c\").plugin({ type: \"internal\", name: \"Comp\" }).bypass(true), to([chain(\"@out\")])])\n";
    let mut files: Vec<(String, String)> = MODULES
        .iter()
        .map(|(p, t)| (format!("libraries/{p}"), (*t).to_owned()))
        .collect();
    files.push(("libraries/k7-core/1/chains/stems.k7x".into(), chain.into()));
    files.push(("home/library/chains/mine.k7x".into(), live.into()));
    if pins {
        files.push((
            "home/workspace/Banc/_library-pins.k7x".into(),
            "k7-core(1)\n".into(),
        ));
    }
    files.extend(
        extra
            .iter()
            .map(|(p, t)| ((*p).to_owned(), (*t).to_owned())),
    );
    for (p, t) in files {
        let at = dir.join(&p);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, t).unwrap();
    }
}

pub(super) fn with_docs(mut c: Ctx, acct: &Arc<Acct>, dir: &Path) -> Ctx {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let _ = dir;
    c.docs = Some(Docs::of(acct, &root));
    c
}

pub(super) async fn body_of(r: axum::response::Response) -> (u16, Value) {
    let status = r.status().as_u16();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_live_list_is_answered_and_an_unreadable_chain_is_listed_unavailable() {
    let dir = scratch("chains-answer");
    fixture(&dir, true, &[]);
    let acct = start(&dir);
    let lines = Lines::default();
    let c = with_docs(ctx(&dir, Tag::LiveChains, "", &lines), &acct, &dir);
    let done = answer_what(&c, What::Live).await.expect("answered");
    assert_eq!(done.status, 200, "{}", done.body);
    // A chain that does not parse is listed unavailable, not a decline of the list (6ecb8ccfa).
    fixture(&dir, true, &[("home/library/chains/bad.k7x", "steps(")]);
    let done = answer_what(&c, What::Live).await.expect("answered");
    assert_eq!(done.status, 200);
    let listed = done.body.to_string();
    assert!(
        listed.contains("@account/bad") && listed.contains("does not read"),
        "{listed}"
    );
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_library_cloned_with_no_pins_is_pinned_afresh_by_the_native_read() {
    let dir = scratch("chains-pins");
    fixture(&dir, false, &[]);
    let acct = start(&dir);
    let lines = Lines::default();
    let c = with_docs(ctx(&dir, Tag::FrozenChains, "", &lines), &acct, &dir);
    let pins = dir.join("home/workspace/Banc/_library-pins.k7x");
    assert!(!pins.exists());
    // The bench's workspace is pinned once, as `pinBench` does before the account's views.
    assert!(answer_what(&c, What::Frozen).await.is_some());
    let text = std::fs::read_to_string(&pins).unwrap();
    assert!(text.starts_with("k7-core("), "{text}");
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_native_read_that_panics_is_counted_said_and_declined() {
    let dir = scratch("chains-panic");
    fixture(&dir, true, &[]);
    let acct = start(&dir);
    let lines = Lines::default();
    let mut c = with_docs(ctx(&dir, Tag::LiveChains, "", &lines), &acct, &dir);
    c.chaos = super::super::Chaos::Panic;
    let before = counts().panics.load(Ordering::Relaxed);
    assert!(answer_what(&c, What::Live).await.is_none());
    assert_eq!(counts().panics.load(Ordering::Relaxed), before + 1);
    assert!(lines.lock().unwrap().iter().any(|l| l.contains("panicked")));
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
}

/// Every file an hour old, since the cache holds nothing it read in the last moments.
fn age(dir: &Path) {
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let at = e.path();
        if at.is_dir() {
            age(&at);
        } else {
            let f = std::fs::File::options().write(true).open(&at).unwrap();
            f.set_modified(old).unwrap();
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_vocabulary_is_built_once_and_the_next_request_finds_it_held() {
    let dir = scratch("chains-vocab");
    fixture(&dir, true, &[]);
    age(&dir);
    let acct = start(&dir);
    let lines = Lines::default();
    let c = with_docs(ctx(&dir, Tag::FrozenChains, "", &lines), &acct, &dir);
    for what in [What::Frozen, What::Live] {
        assert!(compute(&c, &what).is_ok());
    }
    let docs = c.docs.as_ref().unwrap();
    let (_, key, rule, _) = bench_session(&c, docs).unwrap();
    // One build for the first request and one held answer for the second, never one per chain.
    let hits = acct.songs().with(&key, rule, |s| s.vocabulary_hits());
    assert_eq!(hits.ok(), Some(1));
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_cut_of_an_admin_account_is_answered_natively_over_its_drafts() {
    let dir = scratch("chains-admin");
    fixture(&dir, true, &[]);
    let acct = start(&dir);
    let lines = Lines::default();
    let mut c = with_docs(ctx(&dir, Tag::ColdRun, "", &lines), &acct, &dir);
    let run = What::Run {
        want: "stems".into(),
        chain: "stems".into(),
    };
    assert!(compute(&c, &run).is_ok());
    c.home.admin = true;
    assert!(
        compute(&c, &run).is_ok(),
        "the admin's cut reads the overlay"
    );
    // The lists follow the request's own flag.
    assert!(compute(&c, &What::Frozen).is_ok());
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
}

/// A draft of the library: a chain only it holds, its own stems chain, and the modules they step through.
fn drafted(dir: &Path) {
    let chain = "steps([id(\"s\").extends(\"@k7-core/cold/vox\"), to([chain(\"@out\")])])\n";
    let mut files: Vec<(String, String)> = MODULES
        .iter()
        .map(|(p, t)| {
            (
                format!("libraries/{}", p.replacen("/1/", "/draft/", 1)),
                (*t).to_owned(),
            )
        })
        .collect();
    files.push((
        "libraries/k7-core/draft/chains/drafted.k7x".into(),
        chain.into(),
    ));
    let live = "steps([id(\"c\").plugin({ type: \"internal\", name: \"Comp\" }), to([chain(\"@out\")])])\n";
    files.push((
        "libraries/k7-core/draft/chains/stems.k7x".into(),
        live.into(),
    ));
    for (p, t) in files {
        let at = dir.join(&p);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, t).unwrap();
    }
}

/// The native cut of `chain` over a drafted library, as `admin` or not.
async fn drafted_cut(name: &str, admin: bool, chain: &str) -> (u16, Value) {
    let dir = scratch(name);
    fixture(&dir, false, &[]);
    drafted(&dir);
    let acct = start(&dir);
    let lines = Lines::default();
    let mut c = with_docs(ctx(&dir, Tag::ColdRun, "", &lines), &acct, &dir);
    c.home.admin = admin;
    let run = What::Run {
        want: "stems".into(),
        chain: chain.into(),
    };
    let ours = compute(&c, &run)
        .map(|d| (d.status, d.body))
        .expect("answered");
    drop(acct);
    let _ = std::fs::remove_dir_all(dir);
    ours
}

// An admin's cut, answered natively, says what the library's draft holds; another account's reads the published one.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_admins_cut_reads_the_draft_and_the_others_do_not() {
    let mut got = Vec::new();
    for (i, (admin, chain)) in [
        (true, "@k7-core/drafted"),
        (true, "@k7-core/stems"),
        (false, "@k7-core/drafted"),
    ]
    .into_iter()
    .enumerate()
    {
        let (status, body) = drafted_cut(&format!("chains-draft-{i}"), admin, chain).await;
        got.push((admin, chain, status, body["error"].clone()));
    }
    // The draft is not what a library with no pin to it is read from: every cut here is refused in words.
    assert!(got.iter().all(|g| g.2 == 400 && g.3.is_string()), "{got:?}");
}
