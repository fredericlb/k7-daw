//! tests_git_shared_key.rs — a Mac with iCloud: its key joins the shared one, the shared key reaches a signed-in
//! forge on the next accounts read, `GET /git/key` names the old key, and forgetting it drops that key alone.

use std::sync::Arc;

use k7_git::auth::{self, MemorySecrets, Secrets, Synced, is_shared};
use serde_json::json;

use super::git::Kind;
use super::tests_git::{ask, post};
use super::tests_git_forgejo::{added, sign_in};

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..100 {
        if done() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shared_key_reaches_a_signed_in_forge_and_the_old_key_is_named_until_forgotten() {
    let (mut d, f, id) = added("fj-shared-key").await;
    let (synced, local) = (MemorySecrets::default(), MemorySecrets::default());
    let own = auth::public_key(&local).unwrap();
    d.ctx.secrets = Arc::new(Synced {
        synced: synced.clone(),
        local: local.clone(),
        travels: is_shared,
        keeps_old: true,
    });
    sign_in(&d, &id).await;
    assert_eq!(auth::public_key(&*d.ctx.secrets).unwrap(), own, "promoted");
    assert_eq!(f.lock().unwrap().keys.len(), 1);
    // The iPhone's key wins in iCloud: the Mac now signs in with it and keeps its own as the old key.
    let phone = MemorySecrets::default();
    let shared = auth::public_key(&phone).unwrap();
    let kept = phone.get_for("device-ssh-key", "k7").unwrap();
    synced.set("device-ssh-key", "k7", &kept).unwrap();
    let (s, _, _) = ask(&d, Kind::Forges, "GET", "/git/forges", None).await;
    assert_eq!(s, 200);
    until(|| f.lock().unwrap().keys.len() == 2).await;
    assert_eq!(
        f.lock().unwrap().keys.len(),
        2,
        "the shared key is uploaded"
    );
    let (s, k, _) = ask(&d, Kind::Key, "GET", "/git/key", None).await;
    assert_eq!(s, 200);
    assert_eq!(k["publicKey"], json!(shared));
    assert_eq!(k["synced"], json!(true));
    assert_eq!(k["oldKey"]["publicKey"], json!(own));
    assert_eq!(k["oldKey"]["hosts"], json!([]));
    let (s, v, _) = post(&d, Kind::ForgetOldKey, "/git/forget-old-key", json!({})).await;
    assert_eq!((s, &v["forgotten"]), (200, &json!(true)), "{v}");
    let (_, k, _) = ask(&d, Kind::Key, "GET", "/git/key", None).await;
    assert_eq!(k["publicKey"], json!(shared));
    assert!(k.get("oldKey").is_none(), "{k}");
    assert!(local.get_for("old-device-ssh-key", "k7").is_none());
    assert!(
        synced.get_for("device-ssh-key", "k7").is_some(),
        "the shared key stays"
    );
}
