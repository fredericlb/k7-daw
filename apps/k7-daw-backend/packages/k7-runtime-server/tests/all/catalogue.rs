//! catalogue.rs — the verb and reply sets held to what they mirror: engine-protocol's `sessions.ts` and
//! `version.ts`, and the verbs the handler table and the pairing gate answer.

use k7_runtime_server::wire::catalogue::{
    CLIENT_VERBS, ENGINE_GLOBAL_REPLIES, LOCAL_ONLY, PER_SESSION_REPLIES,
};
use k7_runtime_server::wire::version::{ENGINE_PROTOCOL, build};
use std::collections::BTreeSet;

/// The quoted strings of the TypeScript array `name = [ … ]`, comments skipped.
fn ts_array(source: &str, name: &str) -> Vec<String> {
    let start = source
        .find(&format!("{name} = ["))
        .unwrap_or_else(|| panic!("no {name}"));
    let body = &source[start
        ..source[start..]
            .find("] as const")
            .map(|e| start + e)
            .unwrap()];
    body.lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .flat_map(|l| {
            l.split('"')
                .skip(1)
                .step_by(2)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn the_reply_sets_are_sessions_ts() {
    let ts = crate::repo_file("apps/k7-daw-frontend/packages/engine-protocol/sessions.ts");
    assert_eq!(
        ts_array(&ts, "ENGINE_GLOBAL_REPLIES"),
        ENGINE_GLOBAL_REPLIES
    );
    assert_eq!(ts_array(&ts, "PER_SESSION_REPLIES"), PER_SESSION_REPLIES);
}

#[test]
fn the_version_is_version_ts() {
    let ts = crate::repo_file("apps/k7-daw-frontend/packages/engine-protocol/version.ts");
    assert!(ts.contains(&format!(
        "export const ENGINE_PROTOCOL = {ENGINE_PROTOCOL};"
    )));
    let package = crate::repo_file("apps/k7-daw-engine/k7-runtime/package.json");
    assert!(
        std::env::var("K7_BUILD").is_ok()
            || package.contains(&format!("\"version\": \"{}\"", build()))
    );
}

#[test]
fn the_client_verbs_are_the_handlers_and_the_gates() {
    let ours: BTreeSet<String> = CLIENT_VERBS.iter().map(|v| v.to_string()).collect();
    let table = k7_runtime_server::server::handlers::table();
    let mut answered: BTreeSet<String> = table.verbs().map(str::to_string).collect();
    answered.extend(["pair-request", "pair"].map(str::to_string));
    answered.extend(LOCAL_ONLY.iter().map(|v| v.to_string()));
    assert_eq!(ours, answered);
    assert_eq!(ours.len(), CLIENT_VERBS.len(), "a verb listed twice");
}
