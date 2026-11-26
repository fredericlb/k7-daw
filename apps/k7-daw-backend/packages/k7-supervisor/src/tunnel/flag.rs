//! flag.rs — waiting for a `watch<bool>` to be raised, as a future that holds no borrow of the channel across the
//! await (`watch::Ref` is not `Send`). A dropped sender counts as raised: whoever set the flag is gone.

use tokio::sync::watch;

/// Returns once the flag is true, or its sender is gone.
pub async fn raised(rx: &mut watch::Receiver<bool>) {
    let _ = rx.wait_for(|v| *v).await.map(|_| ());
}
