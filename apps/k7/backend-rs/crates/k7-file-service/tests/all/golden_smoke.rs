//! The golden smoke: every project of the staged corpus compiles natively, and the two broken ones name their files.
//! What a reader gets wrong: it is a smoke test, not a byte referee, and it is ignored without `K7_GOLDEN_STAGED` (a dir from `packages/song/scripts/stageCorpus.ts`).
//! A staged tree serves one run; stage-gates.sh stages a fresh one.

use std::collections::BTreeMap;
use std::path::PathBuf;

use k7_file_service::confine::Disk;
use k7_file_service::libraries::Home;
use k7_file_service::rig::empty_rig;
use k7_file_service::song_ask::{Input, song_roots};
use serde_json::Value;
use song::DiskSource;
use song::doc::{ProjectDoc, Vsts};
use song::graph::Folder;
use song::session::{Ask, Session, View};

/// The two projects that must refuse to compile, with the file each is located at.
const BROKEN: [(&str, &str); 2] = [
    ("e2e/broken-sequence.k7", "sequences/melody.k7x"),
    ("e2e/broken-unresolved.k7", "nodes/ghost.k7x"),
];

fn has_null(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.iter().any(has_null),
        Value::Object(o) => o.values().any(has_null),
        _ => false,
    }
}

/// What one project's compile gave: the engine view's text, or the refusal's message; and whether the errors view lists any.
fn compile(
    manifest: &Value,
    home: &Home,
    env: &BTreeMap<String, String>,
    abs: &str,
) -> (Result<Value, String>, bool) {
    let code_root = std::env::var("K7_GOLDEN_REPO").unwrap_or_else(|_| {
        let here = env!("CARGO_MANIFEST_DIR");
        format!("{here}/../../../../..")
    });
    let code_root = std::fs::canonicalize(code_root)
        .expect("the repo")
        .to_string_lossy()
        .into_owned();
    let input = Input {
        home,
        project: abs,
        code_root: &code_root,
        env,
    };
    let roots = song_roots(&input, &Disk).expect("the roots");
    let (mounts, files) = roots.mounts().expect("the mounts");
    let disk = DiskSource::with_files(&mounts, &files).expect("the disk");
    let ask = Ask {
        rig: empty_rig(),
        roots: roots.clone(),
        vsts: Vsts::None,
        thawed: Vec::new(),
        rev: None,
        folder: Folder::Open,
    };
    let _ = manifest;
    let mut session = Session::new(roots.paths);
    let engine = session
        .answer(&disk, &ask, &[View::Engine])
        .map_err(|e| e.to_string())
        .and_then(|r| {
            let text = r
                .views
                .into_iter()
                .find(|(v, _)| *v == View::Engine)
                .map(|(_, t)| t)
                .ok_or("no engine view")?;
            let view = serde_json::from_str::<Value>(&text).map_err(|e| e.to_string())?;
            // A project that does not compile has no document: its view says why.
            if view.get("doc").is_none_or(Value::is_null) {
                Err(text.to_string())
            } else {
                Ok(view)
            }
        });
    let errors = session
        .answer(&disk, &ask, &[View::Errors])
        .ok()
        .and_then(|r| {
            r.views
                .into_iter()
                .find(|(v, _)| *v == View::Errors)
                .map(|(_, t)| t)
        })
        .and_then(|t| serde_json::from_str::<Value>(&t).ok());
    let listed = errors.as_ref().is_some_and(|e| {
        e["errors"].as_array().is_some_and(|l| !l.is_empty())
            || e.as_array().is_some_and(|l| !l.is_empty())
    });
    (engine, listed)
}

#[test]
#[ignore = "needs a staged corpus: K7_GOLDEN_STAGED=<dir from packages/song/scripts/stageCorpus.ts>"]
fn the_staged_corpus_compiles_natively_and_the_two_broken_name_their_files() {
    let dir = PathBuf::from(std::env::var("K7_GOLDEN_STAGED").expect("K7_GOLDEN_STAGED"));
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("manifest.json")).expect("manifest"),
    )
    .expect("JSON");
    let env: BTreeMap<String, String> = manifest["env"]
        .as_object()
        .expect("env")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_owned()))
        .collect();
    let home = Home::of(&env, false);
    let (mut decoded, mut refused) = (Vec::new(), Vec::new());
    for p in manifest["projects"].as_array().expect("projects") {
        let abs = p["abs"].as_str().expect("abs");
        let project = p["project"].as_str().expect("project").to_owned();
        let (engine, listed) = compile(&manifest, &home, &env, abs);
        match engine {
            Ok(view) => {
                let doc = view["doc"].clone();
                assert!(!has_null(&doc), "{project}: a null in the engine document");
                if let Err(e) = serde_json::from_value::<ProjectDoc>(doc) {
                    panic!("{project}: the engine's reader would refuse it: {e}");
                }
                decoded.push(project.clone());
            }
            Err(said) => refused.push((project.clone(), said)),
        }
        assert_eq!(
            listed,
            project.starts_with("e2e/broken-"),
            "{project}: errors listed"
        );
    }
    let names: Vec<&str> = refused.iter().map(|(p, _)| p.as_str()).collect();
    let want: Vec<&str> = BROKEN.iter().map(|(p, _)| *p).collect();
    assert_eq!(
        names, want,
        "the refused projects are exactly the broken two"
    );
    for ((project, said), (_, file)) in refused.iter().zip(BROKEN) {
        assert!(said.contains(file), "{project} should name {file}: {said}");
    }
    eprintln!(
        "corpus: {} documents decoded, {} refused",
        decoded.len(),
        refused.len()
    );
    assert!(
        decoded.len() >= 172 && refused.len() == 2,
        "{} decoded, {} refused",
        decoded.len(),
        refused.len()
    );
}
