//! L1: what the LFS client sends in a batch and how it stops when the host refuses, fails or lies, over SSH.
//! A push that stops here stops before the ref moves: the host's `git-receive-pack` is never reached.

mod common;

use std::sync::atomic::Ordering;

use common::lfs_rig::{SshLfs, ssh_lfs};
use common::{Dir, have_git, me, workspace};
use k7_git::{Error, Repo};

async fn rig() -> SshLfs {
    ssh_lfs("lfs-faults", true).await
}

fn with_a_take() -> (Dir, Repo, String) {
    let (w, repo) = workspace(&[]);
    w.write("take.wav", b"RIFF-a-take-of-audio");
    repo.commit("add take", &me(), "Mac").unwrap();
    let oid = repo.lfs_head().unwrap()[0].1.oid.clone();
    (w, repo, oid)
}

async fn push(r: &SshLfs, w: &Dir) -> Result<(), Error> {
    r.net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
}

fn ref_moved(r: &SshLfs) -> bool {
    r.ref_moved()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_names_the_operation_the_basic_transfer_sha256_and_each_object_with_its_size() {
    if !have_git("lfs-ask") {
        return;
    }
    let r = rig().await;
    let (w, _repo, oid) = with_a_take();
    push(&r, &w).await.unwrap();
    let into = Dir::new("ask-into");
    r.net
        .clone_into(r.to(), into.path().join("c"))
        .await
        .unwrap();
    let asks = r.web.lfs.asks.lock().unwrap().clone();
    let up = &asks[0];
    assert_eq!(up["operation"], "upload");
    assert_eq!(up["transfers"], serde_json::json!(["basic"]));
    assert_eq!(up["hash_algo"], "sha256");
    assert_eq!(
        up["objects"],
        serde_json::json!([{ "oid": oid, "size": 20 }])
    );
    assert!(asks.iter().any(|a| a["operation"] == "download"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_object_the_host_cannot_take_stops_the_push_before_the_ref() {
    if !have_git("lfs-quota") {
        return;
    }
    let r = rig().await;
    r.web.lfs.upload_error.store(true, Ordering::SeqCst);
    let (w, _repo, _oid) = with_a_take();
    let err = push(&r, &w).await.unwrap_err();
    assert!(
        matches!(&err, Error::Network(m) if m.contains("quota")),
        "{err:?}"
    );
    assert!(!ref_moved(&r), "the ref stayed where it was");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_upload_stops_the_push_before_the_ref() {
    if !have_git("lfs-put") {
        return;
    }
    let r = rig().await;
    r.web.lfs.put_status.store(500, Ordering::SeqCst);
    let (w, _repo, oid) = with_a_take();
    let err = push(&r, &w).await.unwrap_err();
    assert!(matches!(err, Error::Network(_)), "{err:?}");
    assert!(!r.web.lfs.has(&oid));
    assert!(!ref_moved(&r), "the ref stayed where it was");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_verify_stops_the_push_before_the_ref() {
    if !have_git("lfs-verify") {
        return;
    }
    let r = rig().await;
    r.web.lfs.verify_status.store(500, Ordering::SeqCst);
    let (w, _repo, _oid) = with_a_take();
    let err = push(&r, &w).await.unwrap_err();
    assert!(matches!(err, Error::Network(_)), "{err:?}");
    assert!(!ref_moved(&r), "the ref stayed where it was");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_grant_the_lfs_endpoint_refuses_is_auth_after_one_request_and_the_ref_stays() {
    if !have_git("lfs-grant") {
        return;
    }
    let r = rig().await;
    *r.web.lfs.require_auth.lock().unwrap() = Some("Bearer nope".into());
    let (w, _repo, _oid) = with_a_take();
    let err = push(&r, &w).await.unwrap_err();
    assert!(matches!(err, Error::Auth(_)), "{err:?}");
    let batches = r
        .web
        .lfs
        .hits
        .lock()
        .unwrap()
        .iter()
        .filter(|h| h.method == "POST")
        .count();
    assert_eq!(batches, 1, "the grant is not tried twice");
    assert!(!ref_moved(&r));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_forbidden_batch_is_auth_and_is_not_asked_again() {
    if !have_git("lfs-403") {
        return;
    }
    let r = rig().await;
    r.web.lfs.batch_status.store(403, Ordering::SeqCst);
    let (w, _repo, _oid) = with_a_take();
    let err = push(&r, &w).await.unwrap_err();
    assert!(matches!(err, Error::Auth(_)), "{err:?}");
    assert_eq!(r.web.lfs.hits.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_transfer_k7_does_not_speak_is_refused_and_nothing_goes_up() {
    if !have_git("lfs-transfer") {
        return;
    }
    let r = rig().await;
    *r.web.lfs.transfer.lock().unwrap() = Some("tus".into());
    let (w, _repo, oid) = with_a_take();
    let err = push(&r, &w).await.unwrap_err();
    assert!(
        matches!(&err, Error::Network(m) if m.contains("transfer")),
        "{err:?}"
    );
    assert!(!r.web.lfs.has(&oid));
    assert!(!ref_moved(&r));
}
