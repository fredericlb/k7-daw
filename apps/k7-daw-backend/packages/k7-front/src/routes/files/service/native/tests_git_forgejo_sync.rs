//! tests_git_forgejo_sync.rs — a Forgejo sign-in shared with another device through the keychain: the read again
//! before a refresh, the one retry after a refusal, and the server list over a roaming store.
//! What a reader gets wrong: the other device is the same keychain seen late (`Late`), not a second front.

use std::io;
use std::sync::{Arc, Mutex};

use k7_git::auth::Secrets;
use k7_roam::{Fake, ListStore, NoStore};
use serde_json::{Value, json};

use super::git::Kind;
use super::git_forge_routes::Verb;
use super::git_forgejo_servers::{Server, forget_in, keep_in, list_in};
use super::tests_git::ask;
use super::tests_git_forgejo::{added, kept, on, sign_in};

/// A keychain whose first `reads` reads of the token answer what it held before another device refreshed.
struct Late {
    inner: Arc<dyn Secrets>,
    old: Mutex<Option<String>>,
    reads: Mutex<usize>,
}

impl Secrets for Late {
    fn get(&self, host: &str, user: &str) -> io::Result<Option<String>> {
        let mut left = self.reads.lock().unwrap();
        if host.starts_with("forge:") && *left > 0 {
            *left -= 1;
            return Ok(self.old.lock().unwrap().clone());
        }
        self.inner.get(host, user)
    }
    fn set(&self, host: &str, user: &str, password: &str) -> io::Result<()> {
        self.inner.set(host, user, password)
    }
    fn delete(&self, host: &str, user: &str) -> io::Result<()> {
        self.inner.delete(host, user)
    }
}

/// Signed in with tokens that are always stale, then refreshed once "on another device": answers the old entry.
async fn refreshed_elsewhere(
    name: &str,
    keep_on_reuse: bool,
) -> (
    super::tests_git::Dev,
    super::tests_git_forgejo_fake::Fake,
    String,
    Value,
) {
    let (d, f, id) = added(name).await;
    {
        let mut st = f.lock().unwrap();
        st.ttl = 30;
        st.keep_on_reuse = keep_on_reuse;
    }
    sign_in(&d, &id).await;
    let old = kept(&d, &id);
    let (s, _, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!(s, 200);
    assert_ne!(kept(&d, &id)["refresh"], old["refresh"]);
    (d, f, id, old)
}

fn late(d: &mut super::tests_git::Dev, old: &Value, reads: usize) {
    d.ctx.secrets = Arc::new(Late {
        inner: d.ctx.secrets.clone(),
        old: Mutex::new(Some(old.to_string())),
        reads: Mutex::new(reads),
    });
}

#[tokio::test(flavor = "multi_thread")]
async fn the_read_under_the_lock_finds_the_token_another_device_refreshed() {
    let (mut d, f, id, old) = refreshed_elsewhere("fj-sync-read", false).await;
    late(&mut d, &old, 1);
    let (s, v, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!(s, 200, "{v}");
    let now = kept(&d, &id);
    assert_eq!(
        f.lock().unwrap().refresh.as_deref(),
        now["refresh"].as_str()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_reads_again_and_retries_once_with_the_newer_token() {
    let (mut d, f, id, old) = refreshed_elsewhere("fj-sync-retry", true).await;
    late(&mut d, &old, 2);
    let (s, v, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!(s, 200, "{v}");
    let now = kept(&d, &id);
    assert_eq!(
        f.lock().unwrap().refresh.as_deref(),
        now["refresh"].as_str()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reuse_that_revokes_the_grant_ends_in_sign_in_again_after_the_retry() {
    let (mut d, f, id, old) = refreshed_elsewhere("fj-sync-revoked", false).await;
    late(&mut d, &old, 2);
    let (s, v, _) = ask(&d, on(&id, Verb::Repos), "GET", "/repos", None).await;
    assert_eq!((s, &v["code"]), (401, &json!("forgejo-signout")));
    assert!(kept(&d, &id).is_null(), "the shared entry is dropped");
    assert!(f.lock().unwrap().refresh.is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sign_in_kept_in_memory_says_it_does_not_sync() {
    let (d, _f, id) = added("fj-sync-flag").await;
    sign_in(&d, &id).await;
    let (_, v, _) = ask(&d, super::git::Kind::Forges, "GET", "/git/forges", None).await;
    let fj = v["forges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["id"] == json!(id))
        .unwrap()
        .clone();
    assert_eq!(fj["synced"], json!(false));
}

fn server(id: &str, client: &str) -> Server {
    Server {
        id: id.into(),
        url: format!("https://{id}"),
        client_id: client.into(),
        ssh_host: id.into(),
        ssh_port: 22,
        page_size: 50,
    }
}

fn home(name: &str) -> String {
    let dir =
        super::tests::tmp_root().join(format!("k7-front-forges-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    super::tests::slashed(&dir)
}

#[test]
fn the_server_list_syncs_through_the_store_and_the_file_is_its_copy() {
    let roam = Fake::default();
    let (a, b) = (home("a"), home("b"));
    keep_in(&roam, &a, server("one.example", "c1")).unwrap();
    assert_eq!(list_in(&roam, &b), [server("one.example", "c1")]);
    assert_eq!(
        list_in(&NoStore, &b),
        [server("one.example", "c1")],
        "the file holds the copy"
    );
    keep_in(&roam, &b, server("one.example", "c2")).unwrap();
    assert_eq!(
        list_in(&roam, &a)[0].client_id,
        "c2",
        "a new client id replaces the server"
    );
    forget_in(&roam, &b, "one.example").unwrap();
    assert!(
        list_in(&roam, &a).is_empty(),
        "a removal elsewhere removes it here"
    );
    assert!(list_in(&NoStore, &a).is_empty());
}

#[test]
fn a_server_kept_before_the_store_existed_is_pushed_to_it() {
    let roam = Fake::default();
    let a = home("before");
    keep_in(&NoStore, &a, server("old.example", "c")).unwrap();
    assert!(roam.pairs().is_empty());
    assert_eq!(list_in(&roam, &a).len(), 1);
    assert_eq!(roam.pairs().len(), 1);
}

fn key_posts(f: &super::tests_git_forgejo_fake::Fake) -> usize {
    let st = f.lock().unwrap();
    st.asked
        .iter()
        .filter(|a| *a == "POST /api/v1/user/keys")
        .count()
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..100 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_synced_sign_in_gets_this_devices_key_uploaded_once_when_the_accounts_are_read() {
    let (d, f, id) = added("fj-own-key").await;
    sign_in(&d, &id).await;
    // As if the token came from another device: the key is not on the host and this device never confirmed it.
    f.lock().unwrap().keys.clear();
    let marker = format!("key-on:{id}");
    let public = k7_git::auth::public_key(&*d.ctx.secrets).unwrap();
    let ours = super::git_forge_token::fingerprint(&public).unwrap();
    d.ctx.secrets.delete(&marker, &ours).unwrap();
    let before = key_posts(&f);
    let (s, _, _) = ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    assert_eq!(s, 200);
    let kept = || d.ctx.secrets.get(&marker, &ours).unwrap().is_some();
    until(kept).await;
    assert!(kept());
    assert_eq!(f.lock().unwrap().keys.len(), 1);
    assert!(!k7_git::auth::is_token(&marker), "the marker never travels");
    let after_first = key_posts(&f);
    assert_eq!(after_first, before + 1);
    ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(
        key_posts(&f),
        after_first,
        "a second read does not upload again"
    );
}
