//! tests_git_key_retry.rs — a call refused for the key on a signed-in forge's host uploads the key and runs once more;
//! on any other host, or with no sign-in, it reaches no forge. What a reader gets wrong: the sshd is not here, the
//! refusal is the operation's own answer (`k7_git::Error::Auth`).

use std::sync::atomic::{AtomicUsize, Ordering};

use k7_git::Error;

use super::git_key_retry::{account_for, retrying};
use super::tests_git::device;
use super::tests_git_forgejo::{added, sign_in};

const REFUSED: &str = "the server refused the device's key for this user";

/// An operation refused `refusals` times, then done.
async fn refused(n: &AtomicUsize, refusals: usize) -> Result<usize, Error> {
    if n.fetch_add(1, Ordering::SeqCst) < refusals {
        return Err(Error::Auth(REFUSED.into()));
    }
    Ok(n.load(Ordering::SeqCst))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_remote_matches_a_signed_in_account_by_its_ssh_host_and_port() {
    let (d, _f, id) = added("key-retry-match").await;
    assert!(
        account_for(&d.ctx, "ssh://git@127.0.0.1/flb/x.git").is_none(),
        "not signed in yet"
    );
    sign_in(&d, &id).await;
    let hit = |u: &str| account_for(&d.ctx, u).map(|f| f.profile().id.clone());
    assert_eq!(hit("ssh://git@127.0.0.1/flb/x.git"), Some(id.clone()));
    assert_eq!(hit("git@127.0.0.1:flb/x.git"), Some(id));
    assert_eq!(
        hit("ssh://git@127.0.0.1:2223/flb/x.git"),
        None,
        "another port"
    );
    assert_eq!(
        hit("ssh://git@elsewhere.test/flb/x.git"),
        None,
        "another host"
    );
    assert_eq!(hit("https://127.0.0.1/flb/x.git"), None, "not ssh");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_on_a_signed_in_host_uploads_the_key_again_and_retries_once() {
    let (d, f, id) = added("key-retry-upload").await;
    sign_in(&d, &id).await;
    // The marker says done, yet the host does not know the key.
    f.lock().unwrap().keys.clear();
    let n = AtomicUsize::new(0);
    let r = retrying(&d.ctx, "ssh://git@127.0.0.1/flb/x.git", || refused(&n, 1)).await;
    assert_eq!(r.unwrap(), 2);
    assert_eq!(f.lock().unwrap().keys.len(), 1, "the key is uploaded again");
    let again = AtomicUsize::new(0);
    let r = retrying(&d.ctx, "ssh://git@127.0.0.1/flb/x.git", || {
        refused(&again, 5)
    })
    .await;
    assert!(matches!(r, Err(Error::Auth(_))));
    assert_eq!(again.load(Ordering::SeqCst), 2, "tried once more, no more");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refusal_with_no_matching_account_calls_no_forge() {
    let (d, f, id) = added("key-retry-none").await;
    sign_in(&d, &id).await;
    let asked = f.lock().unwrap().asked.len();
    let n = AtomicUsize::new(0);
    let other = retrying(&d.ctx, "ssh://git@elsewhere.test/flb/x.git", || {
        refused(&n, 5)
    })
    .await;
    assert!(matches!(other, Err(Error::Auth(_))));
    assert_eq!(n.load(Ordering::SeqCst), 1);
    let bare = device("key-retry-bare");
    let n = AtomicUsize::new(0);
    let r = retrying(&bare.ctx, "ssh://git@127.0.0.1/flb/x.git", || {
        refused(&n, 5)
    })
    .await;
    assert!(matches!(r, Err(Error::Auth(_))));
    assert_eq!(n.load(Ordering::SeqCst), 1);
    assert_eq!(
        f.lock().unwrap().asked.len(),
        asked,
        "the forge was not asked"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_refusal_is_not_retried() {
    let (d, _f, id) = added("key-retry-other").await;
    sign_in(&d, &id).await;
    let n = AtomicUsize::new(0);
    let r: Result<(), Error> = retrying(&d.ctx, "ssh://git@127.0.0.1/flb/x.git", || {
        n.fetch_add(1, Ordering::SeqCst);
        async { Err(Error::Busy) }
    })
    .await;
    assert!(matches!(r, Err(Error::Busy)));
    assert_eq!(n.load(Ordering::SeqCst), 1);
}
