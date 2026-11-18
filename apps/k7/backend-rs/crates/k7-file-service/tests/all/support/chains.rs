//! Shared by the chains tests: a home laid out from `(path, text)` entries, its roots, a disk source and the vocabulary.

#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use k7_file_service::chains::Chains;
use k7_file_service::libraries::Home;
use k7_file_service::listings::families::account_libraries;
use k7_file_service::tree::Limits;
use song::roots::{Library, Roots};
use song::session::Session;
use song::{DiskSource, PathRule, Vocabulary};

use super::Scratch;

pub struct World {
    pub s: Scratch,
    pub home: Home,
    pub roots: Roots,
    pub source: DiskSource,
    pub vocab: Arc<Vocabulary>,
    pub repo: String,
}

pub fn repo() -> String {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    std::fs::canonicalize(at)
        .expect("the repository")
        .to_string_lossy()
        .into_owned()
}

impl World {
    /// The bench's views over `entries`, laid under `studio/`, `libraries/` and `models/` of a scratch folder.
    pub fn new(name: &str, entries: &[(&str, &str)]) -> World {
        let s = Scratch::new(name);
        for (p, t) in entries {
            if p.ends_with('/') {
                s.dir(p);
            } else {
                s.file(p, t);
            }
        }
        let home = s.home(false);
        let bench = s.path("studio/workspace/Banc/live.k7");
        let repo = repo();
        let mut libraries = vec![Library {
            name: "account".into(),
            root: home.account_home(),
            editable: true,
        }];
        let pinned = account_libraries(&home, Some(&bench)).expect("libraries");
        libraries.extend(pinned.into_iter().map(|l| Library {
            name: l.name,
            root: l.root,
            editable: false,
        }));
        let roots = Roots {
            project: bench,
            workspace: Some(s.path("studio/workspace/Banc")),
            confine: Some(s.path("studio/workspace")),
            libraries,
            meta: Some(format!("{repo}/packages/song/meta")),
            models: Some(s.path("models")),
            ..Roots::default()
        };
        let (dirs, files) = roots.mounts().expect("mounts");
        let source = DiskSource::with_files(&dirs, &files).expect("a source");
        let vocab = Session::new(PathRule::HOST)
            .vocabulary(&source, &roots)
            .expect("a vocabulary");
        World {
            s,
            home,
            roots,
            source,
            vocab,
            repo,
        }
    }

    pub fn chains(&self) -> Chains<'_> {
        Chains::new(&self.source, &self.roots, &self.vocab, &Limits::default()).expect("a walk")
    }
}
