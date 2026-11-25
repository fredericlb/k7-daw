// support — a fixture project of e2e/fixtures in memory, a verb run on it, and what the run changed, compiled and undid.
// A reader would get wrong `undo`: it lays the files back as they were before the run, which is what the journal's undo lands.

#![allow(dead_code)]

use k7x::edit::Value;
use song::doc::{ProjectDoc, TrackDoc};
use song_edit::{Fs, MemFs};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use verbs::{Answer, Host, VerbRequest, run_request};

pub const WS: &str = "/w/ws";
const META: &str = "/k7/meta";

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn walk(root: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>) {
    let Ok(entries) = std::fs::read_dir(root.join(rel)) else {
        return;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let sub = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        if e.path().is_dir() {
            walk(root, &sub, out);
        } else if let Ok(bytes) = std::fs::read(e.path()) {
            out.push((sub, bytes));
        }
    }
}

/// Every file by path, as bytes.
pub type Files = BTreeMap<String, Vec<u8>>;

/// What one line did: the verb's answer and the files either side of it.
pub struct Ran {
    pub answer: Answer,
    pub before: Files,
    pub after: Files,
}

impl Ran {
    /// The project-relative paths the run wrote, added or removed.
    pub fn changed(&self, project: &str) -> Vec<String> {
        let prefix = format!("{project}/");
        let mut out: Vec<String> = self
            .after
            .iter()
            .filter(|(p, b)| self.before.get(*p) != Some(*b))
            .map(|(p, _)| p.clone())
            .chain(self.before.keys().filter(|p| !self.after.contains_key(*p)).cloned())
            .map(|p| p.strip_prefix(&prefix).map_or(p.clone(), str::to_owned))
            .collect();
        out.sort();
        out
    }
}

pub struct Project {
    pub fs: MemFs,
    pub dir: String,
    ids: u32,
}

impl Project {
    /// `e2e/fixtures/<name>.k7` with the fixtures' shared `_modules/` beside it and k7's `@meta`.
    pub fn open(name: &str) -> Project {
        let r = repo();
        let mut files = Vec::new();
        let mut add = |from: PathBuf, to: &str| {
            let mut found = Vec::new();
            walk(&from, "", &mut found);
            files.extend(found.into_iter().map(|(rel, b)| (format!("{to}/{rel}"), b)));
        };
        add(r.join(format!("e2e/fixtures/{name}.k7")), &format!("{WS}/{name}"));
        add(r.join("e2e/fixtures/_modules"), &format!("{WS}/_modules"));
        add(r.join("apps/k7-daw-frontend/packages/song/meta"), META);
        Project {
            fs: MemFs::from_tree("", files),
            dir: format!("{WS}/{name}"),
            ids: 0,
        }
    }

    /// The crate's own `tests/fixtures/<name>.k7` (the chain projects e2e has none of), with e2e's `_modules/` and `@meta`.
    pub fn open_crate(name: &str) -> Project {
        let r = repo();
        let mut files = Vec::new();
        let mut add = |from: PathBuf, to: &str| {
            let mut found = Vec::new();
            walk(&from, "", &mut found);
            files.extend(found.into_iter().map(|(rel, b)| (format!("{to}/{rel}"), b)));
        };
        add(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/fixtures/{name}.k7")),
            &format!("{WS}/{name}"),
        );
        add(r.join("e2e/fixtures/_modules"), &format!("{WS}/_modules"));
        add(r.join("apps/k7-daw-frontend/packages/song/meta"), META);
        Project {
            fs: MemFs::from_tree("", files),
            dir: format!("{WS}/{name}"),
            ids: 0,
        }
    }

    pub fn files(&self) -> Files {
        self.fs.files().map(|(p, b)| (p.to_owned(), b.to_vec())).collect()
    }

    pub fn run(&mut self, line: &str) -> Ran {
        let before = self.files();
        let req = VerbRequest {
            line: line.to_owned(),
            project: self.dir.clone(),
            env: serde_json::json!({ "studioHome": "/home-none", "metaRoot": META }),
        };
        let mut n = self.ids;
        let answer = run_request(
            &mut self.fs,
            &mut || {
                n += 1;
                format!("id-{n}")
            },
            &req,
        );
        self.ids = n;
        Ran {
            answer,
            before,
            after: self.files(),
        }
    }

    /// A line that must succeed: its answer, with any refusal or throw failing the test.
    pub fn ok(&mut self, line: &str) -> Ran {
        let ran = self.run(line);
        assert!(
            !ran.answer.refused,
            "`{line}` refused: {:?} {:?}",
            ran.answer.refuse, ran.answer.threw
        );
        ran
    }

    /// A line that must be refused with `why` among its lines, and write nothing.
    pub fn refused(&mut self, line: &str, why: &str) {
        let ran = self.run(line);
        assert!(ran.answer.refused, "`{line}` was not refused: {:?}", ran.answer.log);
        let all = format!(
            "{}\n{}",
            ran.answer.refuse.join("\n"),
            ran.answer.threw.clone().unwrap_or_default()
        );
        assert!(all.contains(why), "`{line}` refused with:\n{all}\nnot `{why}`");
        assert_eq!(ran.before, ran.after, "`{line}` wrote though it refused");
    }

    /// The document the engine is sent for the files as they are now.
    pub fn doc(&mut self) -> ProjectDoc {
        let mut ids = || "id".to_owned();
        let env = song_edit::confine::Env::at("/home-none", META);
        let mut h = Host::new(&mut self.fs, env, &self.dir, &mut ids);
        h.project()
            .unwrap_or_else(|t| panic!("the project does not compile: {}", t.0))
    }

    pub fn track(&mut self, id: &str) -> TrackDoc {
        let doc = self.doc();
        doc.tracks
            .into_iter()
            .find(|t| t.id == id)
            .unwrap_or_else(|| panic!("no track {id}"))
    }

    /// Each event's start in beats on lane `id`, as the engine plays them.
    pub fn starts(&mut self, id: &str) -> Vec<f64> {
        self.track(id).clips.iter().map(|c| c.start).collect()
    }

    pub fn text(&self, rel: &str) -> String {
        self.fs
            .text(&format!("{}/{rel}", self.dir))
            .unwrap_or_else(|| panic!("no file {rel}"))
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.fs.text(&format!("{}/{rel}", self.dir)).is_some()
    }

    /// A k7x file of the project read back as data.
    pub fn data(&self, rel: &str) -> Value {
        song_edit::workspace_doc::ws_data(&self.fs, &format!("{}/{rel}", self.dir)).expect("a k7x file that reads")
    }

    /// Lays the files back as they were before `ran`, as the journal's undo does.
    pub fn undo(&mut self, ran: &Ran) {
        for p in ran.after.keys().filter(|p| !ran.before.contains_key(*p)) {
            self.fs.remove(p).expect("a remove");
        }
        for (p, b) in &ran.before {
            if ran.after.get(p) != Some(b) {
                self.fs.write_atomic(p, b).expect("a write");
            }
        }
    }

    /// Runs `line`, checks it compiles to a different doc, undoes it and checks the doc and files are back.
    pub fn round_trip(&mut self, line: &str) -> Ran {
        let doc = self.doc();
        let ran = self.ok(line);
        self.undo(&ran);
        assert_eq!(self.files(), ran.before, "`{line}`: undo leaves other files");
        assert_eq!(self.doc(), doc, "`{line}`: undo does not give the song back");
        ran
    }
}

/// A list of strings out of a k7x value.
pub fn strs(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::List(l)) => l
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}
