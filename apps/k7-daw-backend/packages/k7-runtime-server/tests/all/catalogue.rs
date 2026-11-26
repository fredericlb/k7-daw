//! catalogue.rs — the verb and reply sets held to what they mirror: engine-protocol's `sessions.ts` and
//! `version.ts`, and, until the C++ server is deleted, the verbs its handler tables answer.

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
fn the_version_is_version_ts_and_the_cpp_servers() {
    let ts = crate::repo_file("apps/k7-daw-frontend/packages/engine-protocol/version.ts");
    assert!(ts.contains(&format!(
        "export const ENGINE_PROTOCOL = {ENGINE_PROTOCOL};"
    )));
    let cpp = crate::repo_file("apps/k7-daw-engine/k7-runtime/server/WireVersion.h");
    assert!(cpp.contains(&format!(
        "constexpr int kEngineProtocol = {ENGINE_PROTOCOL};"
    )));
    let package = crate::repo_file("apps/k7-daw-engine/k7-runtime/package.json");
    assert!(
        std::env::var("K7_BUILD").is_ok()
            || package.contains(&format!("\"version\": \"{}\"", build()))
    );
}

/// Every verb a C++ handler table registers, and every pairing verb its gate answers.
fn cpp_verbs() -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for file in [
        "ServeProject",
        "ServePlugins",
        "ServeMachine",
        "ServeRender",
    ] {
        let src = crate::repo_file(&format!("apps/k7-daw-engine/k7-runtime/server/{file}.cpp"));
        for add in src.split("t.add({").skip(1) {
            let list = add.split('}').next().unwrap_or("");
            out.extend(list.split('"').skip(1).step_by(2).map(str::to_string));
        }
    }
    let pairing = crate::repo_file("apps/k7-daw-engine/k7-runtime/server/ServePairing.cpp");
    for at in pairing.split("type == \"").skip(1) {
        out.insert(at.split('"').next().unwrap_or("").to_string());
    }
    out
}

#[test]
fn the_client_verbs_are_the_cpp_servers() {
    let ours: BTreeSet<String> = CLIENT_VERBS.iter().map(|v| v.to_string()).collect();
    let mut cpp = cpp_verbs();
    cpp.extend(LOCAL_ONLY.iter().map(|v| v.to_string()));
    assert_eq!(ours, cpp);
    assert_eq!(ours.len(), CLIENT_VERBS.len(), "a verb listed twice");
}
