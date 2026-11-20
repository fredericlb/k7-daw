//! docs_failure.rs — what a docs route answers when the compile found no project to read or a project that will not
//! compile: the song crate's findings worded as `compileErrorBody` words a throw, and the empty views of no project.
//! What a reader gets wrong: only three routes turn a failure into a 400; the others have no failing view.

use axum::response::Response;
use serde_json::{Value, json};
use song::session::View;

use crate::routes::files::json_answer;

/// What `requireActive` throws, which the three routes with a try answer as a 400 body.
const NO_ACTIVE: &str = "no active project — select a project in the workspace first";

/// The wording of `FormatTooNewError` for a refusal, which names no file.
fn too_new_message(file_level: u64, build_level: u64) -> String {
    format!(
        "this project was written by a newer k7 (format {file_level}); this k7 reads format {build_level} — update k7 to open it"
    )
}

/// The message of a view's `{"failure":…}` text, as the compile throws it: the findings as lines, or the format refusal.
pub(crate) fn failure_message(text: &str) -> String {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let f = &v["failure"];
    if let (Some(file), Some(build)) = (
        f["formatTooNew"]["fileLevel"].as_u64(),
        f["formatTooNew"]["buildLevel"].as_u64(),
    ) {
        return too_new_message(file, build);
    }
    f["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            let message = d["message"].as_str().unwrap_or("");
            match d["file"].as_str().filter(|file| !file.is_empty()) {
                Some(file) => format!("{file}:{}: {message}", d["line"]),
                None => message.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The 400 body of a view's `{"failure":…}` text: the findings as `<file>:<line>: <message>` lines, or the format refusal.
pub(super) fn compile_failure(text: &str) -> Response {
    let v: Value = serde_json::from_str(text).unwrap_or(Value::Null);
    let f = &v["failure"];
    if let (Some(file), Some(build)) = (
        f["formatTooNew"]["fileLevel"].as_u64(),
        f["formatTooNew"]["buildLevel"].as_u64(),
    ) {
        return json_answer(
            400,
            &json!({
                "error": too_new_message(file, build),
                "code": "format-too-new",
                "fileLevel": file,
                "buildLevel": build,
            }),
        );
    }
    json_answer(400, &json!({ "error": failure_message(text) }))
}

/// A view with no project open: the 400 or 500 of a throw, or the empty answer the route gives (`workspace.ts`).
pub(super) fn no_project(view: View) -> (u16, Value) {
    match view {
        View::Web | View::Engine | View::Graph => (400, json!({ "error": NO_ACTIVE })),
        View::Surfaces => (500, json!({ "error": "internal error" })),
        View::Errors => (200, json!({ "errors": [], "diagnostics": [] })),
        View::Hints => (200, empty_hints()),
        View::IoPorts | View::Catalogue | View::BaseModules => (200, json!([])),
    }
}

/// `EMPTY_HINTS` (`workspaceDocs.ts`), key for key.
fn empty_hints() -> Value {
    json!({
        "baseModules": [], "ioSinks": [], "modules": [], "tracks": [], "channels": [], "clips": [],
        "videoTracks": [], "sequences": [], "chains": [], "moduleAssets": {}, "paramsByBase": {},
        "paramMetaByBase": {}, "racks": {}, "keywords": [], "pluginNames": {}, "paths": {},
        "projectRoot": "", "media": {},
    })
}

/// The line a route with no try says for its 500, as `wrap` does.
pub(super) fn no_project_line(workspace: &str, route: &str) -> String {
    format!("[{workspace}] {route} failed: Error: {NO_ACTIVE}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_is_worded_as_the_compiler_throws_it() {
        let text = r#"{"failure":{"diagnostics":[{"file":"a.k7x","line":3,"message":"bad"},{"file":"","line":0,"message":"loose"}]}}"#;
        let r = compile_failure(text);
        assert_eq!(r.status(), 400);
    }

    #[test]
    fn every_view_has_an_answer_for_no_project() {
        for view in [
            View::Engine,
            View::Web,
            View::Errors,
            View::Hints,
            View::Surfaces,
            View::IoPorts,
            View::Catalogue,
            View::BaseModules,
            View::Graph,
        ] {
            let (status, body) = no_project(view);
            assert!(status == 200 || status >= 400, "{view:?} {body}");
        }
    }
}
