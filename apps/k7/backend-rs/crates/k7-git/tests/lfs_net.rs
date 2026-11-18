//! L1: the batch API over SSH against an in-process LFS server: upload before the ref, eager download, a host
//! without LFS, a foreign host getting no grant, and `git-lfs-authenticate`.

mod common;

use std::sync::atomic::Ordering;

use common::httpd::Httpd;
use common::lfs_rig::{GRANT, SshLfs, ssh_lfs};
use common::lfsd::LfsState;
use common::{Dir, have_git, me, workspace};
use k7_git::lfs::store::Store;
use k7_git::{Error, Repo};

fn wav(w: &Dir, repo: &Repo, name: &str, bytes: &[u8]) -> k7_git::Pointer {
    w.write(name, bytes);
    repo.commit(&format!("add {name}"), &me(), "Mac").unwrap();
    repo.lfs_head()
        .unwrap()
        .into_iter()
        .find(|(p, _)| p == name)
        .unwrap()
        .1
}

async fn push(r: &SshLfs, w: &Dir) -> Result<(), Error> {
    r.net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
}

#[tokio::test(flavor = "multi_thread")]
async fn the_bytes_go_up_before_the_ref_and_come_down_with_a_clone() {
    if !have_git("lfs-ssh-up") {
        return;
    }
    let r = ssh_lfs("lfs-up", true).await;
    let (w, repo) = workspace(&[]);
    let p = wav(&w, &repo, "take.wav", b"RIFF-a-take-of-audio");
    push(&r, &w).await.unwrap();
    assert!(r.web.lfs.has(&p.oid), "the host holds the object");
    let hits = r.web.lfs.hits.lock().unwrap().clone();
    assert!(
        hits.iter()
            .any(|h| h.method == "PUT" && h.path.ends_with(&p.oid)),
        "an upload: {hits:?}"
    );
    assert!(
        hits.iter().any(|h| h.path == "/lfs-verify"),
        "a verify: {hits:?}"
    );
    assert!(r.ref_moved(), "then the ref");
    let into = Dir::new("into");
    r.net
        .clone_into(r.to(), into.path().join("c"))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(into.path().join("c/take.wav")).unwrap(),
        b"RIFF-a-take-of-audio"
    );
    assert!(
        r.web
            .lfs
            .hits
            .lock()
            .unwrap()
            .iter()
            .any(|h| h.method == "GET" && h.path.ends_with(&p.oid))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_host_without_lfs_refuses_the_push_before_anything_is_sent_and_connect_warns_once() {
    if !have_git("lfs-none") {
        return;
    }
    let r = ssh_lfs("lfs-none", false).await;
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    let err = push(&r, &w).await.unwrap_err();
    assert_eq!(
        err,
        Error::NoLfs {
            host: "127.0.0.1".into()
        }
    );
    assert_eq!(err.code(), "no-lfs");
    assert!(!r.ref_moved(), "no ref moved");
    assert!(r.web.lfs.objects.lock().unwrap().is_empty());
    let (net, origin, work) = (r.net.clone(), r.to(), w.path().to_path_buf());
    let probe = r
        .net
        .blocking(move || Ok(Repo::open(&work, &work.join(".git"))?.lfs_probe(&net, &origin)))
        .await
        .unwrap();
    assert!(
        probe.is_some_and(|s| s.contains("127.0.0.1")),
        "connect warns once"
    );
}

async fn probe(r: &SshLfs, w: &Dir) -> Option<String> {
    let (net, origin, work) = (r.net.clone(), r.to(), w.path().to_path_buf());
    r.net
        .blocking(move || Ok(Repo::open(&work, &work.join(".git"))?.lfs_probe(&net, &origin)))
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_host_whose_endpoint_refuses_the_empty_object_still_speaks_lfs_and_takes_a_push() {
    if !have_git("lfs-empty-404") {
        return;
    }
    let r = ssh_lfs("lfs-empty-404", true).await;
    r.web.lfs.empty_batch_status.store(404, Ordering::SeqCst);
    let (w, repo) = workspace(&[]);
    let p = wav(&w, &repo, "take.wav", b"RIFF-take");
    assert_eq!(probe(&r, &w).await, None, "connect gives no warning");
    push(&r, &w).await.unwrap();
    assert!(r.web.lfs.has(&p.oid));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_host_that_refuses_the_repository_says_so_in_its_own_words() {
    if !have_git("lfs-refused") {
        return;
    }
    let r = ssh_lfs("lfs-refused", true).await;
    *r.sshd.lfs_error.lock().unwrap() = Some("ERROR: Repository not found.\n".into());
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    let err = push(&r, &w).await.unwrap_err();
    assert_eq!(
        err,
        Error::LfsRefused("ERROR: Repository not found.".into())
    );
    assert_eq!(err.code(), "lfs-refused");
    assert_eq!(
        probe(&r, &w).await,
        None,
        "a refusal is no 'no LFS' warning"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_404_on_a_real_batch_names_the_status_and_the_endpoint_and_is_no_missing_lfs() {
    if !have_git("lfs-real-404") {
        return;
    }
    let r = ssh_lfs("lfs-real-404", true).await;
    r.web.lfs.batch_status.store(404, Ordering::SeqCst);
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    let err = push(&r, &w).await.unwrap_err();
    assert!(
        matches!(&err, Error::LfsStatus { status: 404, url } if url.contains("/r.git/info/lfs/objects/batch")),
        "{err:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_action_on_another_host_gets_its_own_headers_and_none_of_the_grant() {
    if !have_git("lfs-foreign") {
        return;
    }
    let r = ssh_lfs("lfs-foreign", true).await;
    let other = Httpd::start_with(r.web.lfs.clone()).await;
    *r.web.lfs.require_auth.lock().unwrap() = None;
    *r.web.lfs.href_base.lock().unwrap() = Some(format!("http://127.0.0.1:{}", other.addr.port()));
    let (w, repo) = workspace(&[]);
    let p = wav(&w, &repo, "take.wav", b"RIFF-take");
    push(&r, &w).await.unwrap();
    assert!(r.web.lfs.has(&p.oid));
    let theirs = r.web.lfs.hits_on(&other.addr.port().to_string());
    assert!(
        theirs.iter().any(|h| h.method == "PUT"),
        "the bytes went to the second host: {theirs:?}"
    );
    assert!(
        theirs.iter().all(|h| h.auth.is_none()),
        "no grant reached it: {theirs:?}"
    );
    let mine = r.web.lfs.hits_on(&r.web.addr.port().to_string());
    assert!(
        mine.iter().any(|h| h.auth.as_deref() == Some(GRANT)),
        "the endpoint itself got the grant: {mine:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn over_ssh_the_grant_of_git_lfs_authenticate_opens_the_endpoint_and_is_reused() {
    if !have_git("lfs-ssh") {
        return;
    }
    let r = ssh_lfs("lfs-ssh", true).await;
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "one.wav", b"RIFF-one");
    push(&r, &w).await.unwrap();
    wav(&w, &repo, "two.wav", b"RIFF-two");
    push(&r, &w).await.unwrap();
    let cmds = r.sshd.commands.lock().unwrap().clone();
    let asked = cmds
        .iter()
        .filter(|c| c.starts_with("git-lfs-authenticate '/r.git' upload"))
        .count();
    assert_eq!(asked, 1, "the grant is reused until it expires: {cmds:?}");
    let hits = r.web.lfs.hits.lock().unwrap().clone();
    assert!(
        hits.len() >= 4 && hits.iter().all(|h| h.auth.as_deref() == Some(GRANT)),
        "{hits:?}"
    );
    assert_eq!(r.web.lfs.objects.lock().unwrap().len(), 2);
    let into = Dir::new("into");
    r.net
        .clone_into(r.to(), into.path().join("c"))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(into.path().join("c/two.wav")).unwrap(),
        b"RIFF-two"
    );
}

#[test]
fn the_store_is_the_one_git_lfs_reads() {
    let p = Store::in_gitdir(std::path::Path::new("/g")).path(&"ab".repeat(32));
    assert!(p.ends_with(format!("lfs/objects/ab/ab/{}", "ab".repeat(32))));
    let _ = LfsState::default();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_that_is_not_the_object_asked_for_is_refused_and_nothing_is_kept() {
    if !have_git("lfs-corrupt") {
        return;
    }
    let r = ssh_lfs("lfs-x", true).await;
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    r.net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
        .unwrap();
    r.web.lfs.corrupt_downloads.store(true, Ordering::SeqCst);
    let into = Dir::new("into");
    let err = r
        .net
        .clone_into(r.to(), into.path().join("c"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Offline { .. } | Error::Network(_)),
        "{err:?}"
    );
    assert!(
        std::fs::read_dir(into.path()).unwrap().next().is_none(),
        "no workspace is left"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_names_a_path_as_an_oid_reads_nothing_of_the_device() {
    if !have_git("lfs-evil-oid") {
        return;
    }
    let r = ssh_lfs("lfs-x", true).await;
    let secret = Dir::new("secret");
    secret.write("id_secret", b"PRIVATE-KEY-BYTES");
    *r.web.lfs.evil_oid.lock().unwrap() =
        Some(secret.path().join("id_secret").display().to_string());
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    let err = r
        .net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Network(_)), "{err:?}");
    assert!(
        r.web
            .lfs
            .objects
            .lock()
            .unwrap()
            .values()
            .all(|b| b != b"PRIVATE-KEY-BYTES"),
        "the secret was uploaded"
    );
    assert!(
        r.web
            .lfs
            .hits
            .lock()
            .unwrap()
            .iter()
            .all(|h| h.method != "PUT"),
        "no object went up"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_action_over_plain_http_away_from_this_machine_is_refused_before_a_byte_goes() {
    if !have_git("lfs-plain-href") {
        return;
    }
    let r = ssh_lfs("lfs-x", true).await;
    *r.web.lfs.require_auth.lock().unwrap() = None;
    *r.web.lfs.href_base.lock().unwrap() = Some("http://storage.example.invalid".into());
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    let err = r
        .net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
        .unwrap_err();
    assert_eq!(err, Error::InsecureUrl);
    assert!(!r.ref_moved(), "no ref moved");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_longer_than_the_size_asked_is_cut_and_leaves_no_temp() {
    if !have_git("lfs-oversize") {
        return;
    }
    let r = ssh_lfs("lfs-x", true).await;
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "take.wav", b"RIFF-take");
    r.net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
        .unwrap();
    r.web.lfs.extra_bytes.store(1_000_000, Ordering::SeqCst);
    let (net, origin, work) = (r.net.clone(), r.to(), w.path().to_path_buf());
    let err = r
        .net
        .blocking(move || {
            let repo = Repo::open(&work, &work.join(".git"))?;
            let files = repo.lfs_head()?;
            repo.lfs_fetch_objects(&net, &origin, &files)
        })
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("more than"),
        "cut at the size, not at the end of the stream: {err}"
    );
    let tmp = w.path().join(".git/lfs/tmp");
    assert!(
        !tmp.exists() || std::fs::read_dir(tmp).unwrap().next().is_none(),
        "no temp is left"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_leaves_an_object_out_is_offline_and_no_file_lands() {
    if !have_git("lfs-drop") {
        return;
    }
    let r = ssh_lfs("lfs-x", true).await;
    let (w, repo) = workspace(&[]);
    wav(&w, &repo, "a.wav", b"RIFF-a");
    wav(&w, &repo, "b.wav", b"RIFF-b");
    r.net
        .push(w.path().to_path_buf(), w.path().join(".git"), r.to())
        .await
        .unwrap();
    *r.web.lfs.drop_item.lock().unwrap() = Some(1);
    let into = Dir::new("into");
    let err = r
        .net
        .clone_into(r.to(), into.path().join("c"))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Offline { .. }), "{err:?}");
    assert!(
        std::fs::read_dir(into.path()).unwrap().next().is_none(),
        "no workspace, so no file landed"
    );
}
