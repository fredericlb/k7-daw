// doc_support/gate.rs — one project compiled as the gate compares it: the engine document folded, the freezes as arrays, the finding files.
// A reader would get wrong that the group fold is applied here and not by the compile: the file service folds at the send (decision 331).

use serde_json::{Value, json};
use song::doc::{Compiled, Request, Spent, Vsts, compile, with_group_mix};
use song::files::Severity;
use song::lanes::rig::Rig;
use song::record::Recorder;
use song::{Cache, DiskSource, MemSource, Roots, Source, SourceError};

/// The message of the one WARNING a disk `Stale` gives and a replay does not (decision 352); the gate alone matches it.
pub const STALE_WORDS: &str = "its graph needs rebuilding";

/// What a compile made, in the shapes the gate compares.
pub struct Ran {
    pub compiled: Compiled,
    /// The folded engine document, or null when the compile failed.
    pub engine: Value,
    pub freezes: Value,
    /// Files with an ERROR among the diagnostics, and files with a WARNING, each once, sorted.
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// The files a failure names.
    pub stopped: Vec<String>,
    pub spent: Spent,
    /// The project's folder, for the counts that read its files.
    pub project: String,
}

pub fn request(roots: &Roots, rig: &Value, scan_none: bool, thawed: &[String]) -> Request {
    Request {
        roots: roots.clone(),
        rig: Rig::from_json(rig),
        vsts: if scan_none { Vsts::None } else { Vsts::Scan },
        thawed: thawed.to_vec(),
    }
}

pub fn files(c: &Compiled, severity: Severity, skip_stale: bool) -> Vec<String> {
    let mut v: Vec<String> = c
        .diagnostics
        .iter()
        .filter(|f| f.severity == severity && !(skip_stale && f.message == STALE_WORDS))
        .map(|f| f.file.clone())
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The AutoFreeze list as JSON: the sets arrays already (decision 359).
pub fn freezes_json(c: &Compiled) -> Value {
    serde_json::to_value(&c.freezes).unwrap_or(Value::Null)
}

pub fn run(source: &dyn Source, cache: &mut Cache, req: &Request) -> Result<Ran, SourceError> {
    let compiled = compile(source, cache, req)?;
    let (engine, stopped) = match &compiled.engine {
        Ok(doc) => {
            let mut folded = doc.clone();
            with_group_mix(&mut folded);
            (serde_json::to_value(&folded).unwrap_or(Value::Null), Vec::new())
        }
        Err(f) => {
            let mut v: Vec<String> = f.diagnostics.iter().map(|d| d.file.clone()).collect();
            v.sort();
            v.dedup();
            (Value::Null, v)
        }
    };
    Ok(Ran {
        engine,
        freezes: freezes_json(&compiled),
        errors: files(&compiled, Severity::Error, false),
        warnings: files(&compiled, Severity::Warning, true),
        stopped,
        spent: compiled.spent,
        project: req.roots.project.clone(),
        compiled,
    })
}

/// What a replay through a bundle must keep: the document, the freezes, the finding files (the stale WARNING aside).
pub fn same_run(a: &Ran, b: &Ran) -> Result<(), String> {
    if a.engine != b.engine {
        return Err("the bundle's document differs from the disk's".to_owned());
    }
    if a.freezes != b.freezes {
        return Err("the bundle's freezes differ from the disk's".to_owned());
    }
    if a.errors != b.errors || a.warnings != b.warnings || a.stopped != b.stopped {
        return Err(format!(
            "the bundle's findings differ: {:?}/{:?} against {:?}/{:?}",
            a.errors, a.warnings, b.errors, b.warnings
        ));
    }
    Ok(())
}

pub fn spent_json(s: &Spent) -> Value {
    json!({ "doc": s.doc, "placer": s.placer, "baker": s.baker, "freeze": s.freeze })
}

/// A project on disk compiled through a Recorder; `renders` names the modules whose render is made (at the name the Rust gives)
/// before it compiles again; the bundle it made replays equal unless `mem` is false. The second is the replay's failures.
pub fn run_disk(
    cache: &mut Cache,
    roots: &Roots,
    rig: &Value,
    vsts_none: bool,
    renders: &[String],
    mem: bool,
) -> Result<(Ran, Vec<String>), SourceError> {
    if let Some(r) = &roots.renders {
        std::fs::create_dir_all(r).map_err(|e| SourceError::Io(e.to_string()))?;
    }
    let req = request(roots, rig, vsts_none, &[]);
    let (mounts, files) = roots.mounts()?;
    let go = |cache: &mut Cache| -> Result<(Ran, Recorder<'_>), SourceError> {
        // The Source outlives the call by being leaked into the Recorder's lifetime through a Box.
        let disk: &'static DiskSource = Box::leak(Box::new(DiskSource::with_files(&mounts, &files)?));
        let rec = Recorder::new(disk);
        let ran = run(&rec, cache, &req)?;
        Ok((ran, rec))
    };
    let (mut ran, mut rec) = go(cache)?;
    if !renders.is_empty() {
        for f in ran.compiled.freezes.iter().filter(|f| renders.contains(&f.module)) {
            std::fs::write(&f.render, b"").map_err(|e| SourceError::Io(e.to_string()))?;
        }
        (ran, rec) = go(cache)?;
    }
    let mut failures = Vec::new();
    if mem {
        let bundle = rec.into_bundle_with_files(&mounts, &files);
        match MemSource::from_bundle(bundle).map(|m| run(&m, &mut Cache::new(), &req)) {
            Ok(Ok(again)) => {
                if let Err(why) = same_run(&ran, &again) {
                    failures.push(why);
                }
            }
            Ok(Err(e)) => failures.push(format!("the bundle was not enough: {e}")),
            Err(e) => failures.push(e.to_string()),
        }
    }
    Ok((ran, failures))
}
