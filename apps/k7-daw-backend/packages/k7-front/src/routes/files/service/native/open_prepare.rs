//! open_prepare.rs — `/open`'s first half, native: the libraries refreshed through the hub, the workspace pinned.
//! What a reader gets wrong: with no native capture the whole step is declined.

use std::sync::Arc;
use std::time::Duration;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::CONTENT_TYPE;
use k7_file_service::libraries::Home;

use super::entries::blocking;
use super::listings::http;
use super::projects::Proj;
use crate::routes::files::service::Who;

/// How long the hub gets to prepare the libraries before the open goes on without them.
const HUB_BOUND: Duration = Duration::from_secs(120);

/// The account's native capture, where the front captures for it.
pub(super) fn capture_of(who: &Who<'_>) -> Option<Arc<k7_capture::account::Account>> {
    let home = Home::of(&who.held.account.env);
    k7_capture::Captures::global().account(&home.studio)
}

/// Why the step has no answer: a decline the route words, or a blocking step that was lost.
pub(super) enum Failure {
    Declined(&'static str),
    Lost,
}

/// The refreshes of one process one after the other, so an older one never lands over a newer flag.
static TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The libraries refreshed and the workspace pinned: `openPrepare`, without a sync gate.
pub(super) async fn prepare(p: &Proj, root: &str) -> Result<(), Failure> {
    if p.capture.is_none() {
        return Err(Failure::Declined("no-capture"));
    }
    #[cfg(test)]
    super::entries::calls::gate(root, "openPrepare").await;
    refresh_libraries(p).await;
    let (home, r) = (p.home.clone(), root.to_string());
    match blocking(move || k7_file_service::pins::ensure_opened(&home, &r)).await {
        Some(Ok(_)) => {}
        Some(Err(d)) => return Err(Failure::Declined(d.as_str())),
        None => return Err(Failure::Lost),
    }
    Ok(())
}

/// `refreshLibraries`: the hub asked to give the libraries their folders; a failure is said, never an open's.
async fn refresh_libraries(p: &Proj) {
    let Some(hub) = &p.hub else { return };
    let _turn = TURN.lock().await;
    if let Err(e) = sync(hub).await {
        p.say("warn", &format!("[libraries] {e}"));
    }
}

async fn sync(hub: &(String, String)) -> Result<(), String> {
    let req = hyper::Request::post(format!("{}/libraries/sync", hub.0))
        .header(CONTENT_TYPE, "application/json")
        .header("x-studio-hub-secret", hub.1.as_str())
        .body(Full::new(Bytes::from("{}")))
        .map_err(|e| e.to_string())?;
    let work = async {
        let res = http().request(req).await.map_err(|e| e.to_string())?;
        let status = res.status();
        if !status.is_success() {
            return Err(format!(
                "the hub could not prepare the libraries ({})",
                status.as_u16()
            ));
        }
        crate::web::collect_within(res.into_body(), 1 << 20)
            .await
            .map(|_| ())
            .map_err(|_| "the hub's answer was too large".to_string())
    };
    tokio::time::timeout(HUB_BOUND, work)
        .await
        .map_err(|_| "the hub did not answer in time".to_string())?
}
