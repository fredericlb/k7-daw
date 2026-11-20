//! workspaces_start.rs — Examples stocked with the bundled demos when an account starts (1642, 1643).
//! What a reader gets wrong: this runs BEFORE the account's runtime exists, so nothing here can ask it; the environment
//! can tell it not to mirror, and a start never fails on it.

use std::collections::BTreeMap;
use std::path::Path;

use k7_file_service::libraries::Home;
use k7_file_service::paths::join;
use k7_file_service::workspaces::EXAMPLES;
use k7_file_service::workspaces::demos::{demos_dir, mirror};
use k7_file_service::workspaces::names::workspace_dir;
use k7_hub::Log;

use super::locks;

/// The demos' folder and Examples' for an account, or none where there is no bundle.
fn places(env: &BTreeMap<String, String>, code_root: &Path) -> (Home, Option<String>, String) {
    let home = Home::of(env);
    let src = demos_dir(
        env.get("STUDIO_DEMOS").map(String::as_str),
        &code_root.display().to_string(),
    );
    let examples = join(&[&workspace_dir(&home), EXAMPLES]);
    (home, src, examples)
}

/// The line the TypeScript says after a mirror.
fn said(synced: &[String]) -> String {
    let what = if synced.is_empty() {
        "demos up to date".to_string()
    } else {
        synced.join(", ")
    };
    format!("   {EXAMPLES}: {what}")
}

/// Examples mirrored here before the account starts; true, so the runtime does not mirror.
pub async fn stocked(env: &BTreeMap<String, String>, code_root: &Path, log: &Log) -> bool {
    let (home, src, examples) = places(env, code_root);
    let _lock = locks::write(&home.studio).await;
    let synced = tokio::task::spawn_blocking(move || {
        // No demos folder is the TypeScript's `demos up to date`: it mirrors nothing and says so.
        src.map(|s| mirror(&s, &examples)).unwrap_or_default()
    })
    .await
    .unwrap_or_default();
    log(&said(&synced));
    true
}
