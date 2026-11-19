//! capture_start.rs — an account's native capture started with its account and stopped with it (2105, 2111): the host the
//! captures run in, made from the account's environment and the front's locks and memos.
//! What a reader gets wrong: the capture starts only for an account the front captures for.

use std::collections::BTreeMap;
use std::sync::Arc;

use k7_capture::account::Host;
use k7_capture::source::Process;
use k7_capture::worker::Timing;
use k7_capture::{Captures, native};
use k7_file_service::libraries::Home;
use k7_file_service::workspaces::names::workspace_dir;
use k7_hub::Log;

use super::capture_bundle::FrontBundle;
use super::engine::routes::Engine;

/// Starts the account's captures: every workspace attached on the runtime, as a start of the account would (2150).
pub async fn start(
    engine: Option<Arc<Engine>>,
    env: &BTreeMap<String, String>,
    slug: &str,
    log: &Log,
    watch: Option<Arc<k7_file_service::modwatch::Watch>>,
) {
    let home = Home::of(env);
    let bundle = FrontBundle::new(&home.studio, slug, watch, engine);
    let studio = home.studio.clone();
    let host = Host {
        home: studio.clone(),
        workspaces: workspace_dir(&home),
        library: home.account_home(),
        source: Arc::new(Process),
        k7x: native(),
        bundle: Arc::new(bundle),
        timing: Timing::default(),
    };
    let say = log.clone();
    let say: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(move |m| say(m));
    let rt = tokio::runtime::Handle::current();
    Captures::global().start_account(say, host, &rt).await;
}

/// Stops an account's captures, each ledger saved; where no runtime is at hand the stop is not waited for.
pub fn stop(env: &BTreeMap<String, String>) {
    let studio = Home::of(env).studio;
    // The account named now, not when the task runs: a start made meanwhile is never the one stopped.
    let Some(acct) = Captures::global().account(&studio) else {
        return;
    };
    if let Ok(rt) = tokio::runtime::Handle::try_current() {
        rt.spawn(async move { Captures::global().stop_this(&studio, &acct).await });
    }
}

/// `stop`, waited: the account's captures have ended and each ledger is saved when this returns.
pub async fn stop_waited(env: &BTreeMap<String, String>) {
    let studio = Home::of(env).studio;
    if let Some(acct) = Captures::global().account(&studio) {
        Captures::global().stop_this(&studio, &acct).await;
    }
}
