// verb_ops.rs — the `verb.run` operation: one command line run by the `verbs` crate over a file system, its answer as JSON.
// A reader would get wrong that a refusal is an error here: it is an answer (`refused: true`); only an unknown file is `needs_files`.

use crate::error::WasmError;
use crate::host_fs::HostFs;
use song_edit::Fs;
use verbs::{Answer, VerbRequest, run_request};

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

    #[test]
    fn an_unported_verb_is_refused_by_name() {
        let mut fs = MemFs::new();
        let req: VerbRequest =
            serde_json::from_value(serde_json::json!({ "line": "picture mute v", "project": "" })).unwrap();
        let a = run_over(&mut fs, &mut || "id".to_owned(), &req);
        assert!(a.refused);
        assert_eq!(a.refuse, ["command 'picture mute' has no handler (picture.mute)"]);
    }
}
