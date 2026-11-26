// verb_ops.rs — the `verb.run` and `verb.complete` operations: a command line run, or completed, by the `verbs` crate over a file system.
// `verb.catalogue` takes no files and is answered in `ops.rs` from the crate's `palette`.
// A reader would get wrong that a refusal is an error here: it is an answer (`refused: true`); only an unknown file is `needs_files`.

use crate::error::WasmError;
use crate::host_fs::HostFs;
use serde::{Deserialize, Serialize};
use song::Cache;
use song_edit::Fs;
use verbs::{Answer, CompleteRequest, Item, VerbRequest, complete_request, run_request};

/// The line run over `fs`: what a native host calls with its own file system.
pub fn run_over(fs: &mut dyn Fs, new_id: &mut dyn FnMut() -> String, req: &VerbRequest) -> Answer {
    run_request(fs, new_id, req)
}

/// The line run over the host's imports; a path it lacked is a `needs_files` answer, whatever the verb said.
pub fn run(req: VerbRequest) -> Result<Answer, WasmError> {
    let mut fs = HostFs::new();
    let answer = run_over(&mut fs, &mut crate::abi::new_id, &req);
    let missing = fs.take_missing();
    if !missing.is_empty() {
        return Err(WasmError::needs_files(missing));
    }
    Ok(answer)
}

/// `verb.complete`'s arguments: the request, and whether a label may come back as an object.
#[derive(Clone, Debug, Deserialize)]
pub struct CompleteArgs {
    #[serde(flatten)]
    pub request: CompleteRequest,
    /// The page's `labelled`: a labelled candidate is `{line, label}`, the others bare lines; false answers bare lines only.
    #[serde(default)]
    pub labelled: bool,
}

/// One candidate as the page's `complete` returns it: a bare line, or `{line, label}` when it has a label to show.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Candidate {
    Line(String),
    Labelled(Item),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CompleteAnswer {
    pub items: Vec<Candidate>,
}

/// The line completed over `fs`; each call reads the files afresh, as a page holding no compile cache does.
pub fn complete_over(fs: &dyn Fs, args: &CompleteArgs) -> CompleteAnswer {
    let items = complete_request(fs, &mut Cache::new(), &args.request);
    CompleteAnswer {
        items: items
            .into_iter()
            .map(|i| match i.label {
                Some(_) if args.labelled => Candidate::Labelled(i),
                _ => Candidate::Line(i.line),
            })
            .collect(),
    }
}

/// The line completed over the host's imports; a path it lacked is a `needs_files` answer, whatever the providers found.
pub fn complete(args: CompleteArgs) -> Result<CompleteAnswer, WasmError> {
    let fs = HostFs::new();
    let answer = complete_over(&fs, &args);
    let missing = fs.take_missing();
    if !missing.is_empty() {
        return Err(WasmError::needs_files(missing));
    }
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use song_edit::MemFs;

    #[test]
    fn a_line_runs_over_a_native_file_system() {
        let mut fs = MemFs::from_tree(
            "/w/p",
            [
                (
                    "project.k7x".to_owned(),
                    b"project({ name: \"p\", tempo: 120 })\n".to_vec(),
                ),
                ("graph.k7x".to_owned(), b"modules({ MIX: [\"OUT\"] })\n".to_vec()),
            ],
        );
        let req: VerbRequest = serde_json::from_value(serde_json::json!({
            "line": "project tempo 96",
            "project": "/w/p",
            "env": { "studioHome": "/none", "metaRoot": "/meta" },
        }))
        .unwrap();
        let a = run_over(&mut fs, &mut || "id".to_owned(), &req);
        assert_eq!(a.log, ["tempo 96 bpm"]);
        assert_eq!(a.effects, ["reload"]);
        assert!(!a.refused);
        assert!(fs.text("/w/p/project.k7x").unwrap().contains("tempo: 96"));
    }

    fn project_fs() -> MemFs {
        MemFs::from_tree(
            "/w/p",
            [
                (
                    "project.k7x".to_owned(),
                    b"project({ name: \"p\", tempo: 120 })\n".to_vec(),
                ),
                ("graph.k7x".to_owned(), b"modules({ MIX: [\"OUT\"] })\n".to_vec()),
            ],
        )
    }

    #[test]
    fn a_line_completes_as_json_through_the_request_enum() {
        let call: crate::ops::Request = serde_json::from_value(serde_json::json!({
            "op": "verb.complete",
            "args": { "line": "project temp", "cursor": 12, "project": "/w/p", "env": { "studioHome": "/none", "metaRoot": "/meta" } },
        }))
        .unwrap();
        let crate::ops::Request::VerbComplete(args) = call else {
            panic!("not a completion")
        };
        let answer = complete_over(&project_fs(), &args);
        assert_eq!(
            serde_json::to_value(&answer).unwrap(),
            serde_json::json!({ "items": ["project tempo"] })
        );
    }

    #[test]
    fn the_catalogue_is_asked_by_its_op_alone() {
        let mut out = Vec::new();
        let status = crate::ops::Session::new().handle(br#"{"op":"verb.catalogue"}"#, &mut out);
        assert_eq!(status, crate::ops::OK);
        let rows: serde_json::Value = serde_json::from_slice(&out).unwrap();
        let rename = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["command"] == "track rename")
            .unwrap();
        assert_eq!(rename["args"][0]["provider"], "tracks");
    }

    #[test]
    fn an_unknown_verb_is_refused_by_name() {
        let mut fs = MemFs::new();
        let req: VerbRequest =
            serde_json::from_value(serde_json::json!({ "line": "frobnicate v", "project": "" })).unwrap();
        let a = run_over(&mut fs, &mut || "id".to_owned(), &req);
        assert!(a.refused);
        assert_eq!(a.refuse, ["unknown command: frobnicate v"]);
    }
}
