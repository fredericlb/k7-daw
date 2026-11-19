//! store.rs — the origin's LIBRARIES: the one list (`<dataRoot>/libraries.json`) and one folder per library under
//! `<dataRoot>/libraries/<name>`, its frozen versions `1/`, `2/`… and the `draft/` only admins write. `libraries.ts`.
//! What a reader gets wrong: every change is copied into a dot-named sibling and renamed, so no reader sees half of one.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use k7_file_service::paths::resolve_all;
use serde_json::{Map, Value, json};

use crate::copy::copy_tree;
use crate::dirs::{
    DRAFT, MANIFEST, UNPINNED_VERSION, is_copy_of_seed, is_husk, publishable, read_manifest,
    version_numbers, write_first_manifest,
};
use crate::list::{Note, library_folders, parse_list, rm, stamp};
use crate::manifest::{Manifest, stringify_manifest};

pub use crate::list::{parse_seed, settings_problem};

/// The library a new install starts from.
pub const DEFAULT_LIBRARY: &str = "k7-core";

pub struct Store {
    /// Where the libraries are: what every file service gets as `$STUDIO_LIBRARIES_HOME`.
    pub home: PathBuf,
    list_file: PathBuf,
    seed: Vec<(String, PathBuf)>,
    // Every change to the folders runs under this: two copies deciding on the same name would collide.
    queue: Mutex<()>,
}

impl Store {
    pub fn new(data_root: &Path, seed: Option<&str>) -> Store {
        let root = PathBuf::from(resolve_all(&[&data_root.to_string_lossy()]));
        Store {
            home: root.join("libraries"),
            list_file: root.join("libraries.json"),
            seed: parse_seed(seed),
            queue: Mutex::new(()),
        }
    }

    fn serial(&self) -> std::sync::MutexGuard<'_, ()> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn seed_of(&self, name: &str) -> Option<&PathBuf> {
        self.seed.iter().find(|(n, _)| n == name).map(|(_, d)| d)
    }

    /// The list; with none written yet, the libraries already on disk, else the seed's.
    pub fn settings(&self) -> Vec<String> {
        if self.list_file.exists() {
            return fs::read(&self.list_file)
                .map(|b| parse_list(&String::from_utf8_lossy(&b)))
                .unwrap_or_default();
        }
        let here = library_folders(&self.home);
        if here.is_empty() {
            self.seed.iter().map(|(n, _)| n.clone()).collect()
        } else {
            here
        }
    }

    /// The list as an account sees it: each library, and whether the server holds its folder.
    pub fn view(&self, notes: &[Note]) -> Value {
        let libraries: Vec<Value> = self
            .settings()
            .iter()
            .map(|n| {
                let versions: Vec<Value> = self
                    .versions(n)
                    .iter()
                    .map(|m| match &m.notes {
                        Some(t) => {
                            json!({ "version": m.version, "requires": m.requires, "notes": t })
                        }
                        None => json!({ "version": m.version, "requires": m.requires }),
                    })
                    .collect();
                json!({
                    "name": n,
                    "present": self.home.join(n).exists(),
                    "versions": versions,
                    "draft": self.home.join(n).join(DRAFT).exists(),
                })
            })
            .collect();
        let mut reply = Map::new();
        reply.insert("libraries".into(), Value::Array(libraries));
        if !notes.is_empty() {
            reply.insert("notes".into(), notes.iter().map(Note::json).collect());
        }
        Value::Object(reply)
    }

    /// Give every listed library (or only `name`) its folder, and its draft when asked.
    pub fn ensure(&self, name: Option<&str>, draft: bool) -> Vec<Note> {
        let _q = self.serial();
        let mut notes = Vec::new();
        for l in self.settings() {
            if name.is_some_and(|n| n != l) {
                continue;
            }
            let note = self.ensure_one(&l);
            if !draft || note.state == "failed" {
                notes.push(note);
            } else {
                notes.push(self.ensure_draft(&l).unwrap_or(note));
            }
        }
        notes
    }

    /// The published versions of library `name`, oldest first, each as its `library.k7x` says.
    pub fn versions(&self, name: &str) -> Vec<Manifest> {
        let at = self.home.join(name);
        version_numbers(&at)
            .into_iter()
            .map(|v| {
                let dir = at.join(v.to_string());
                read_manifest(&dir).unwrap_or_else(|| Manifest::plain(name, v, 1))
            })
            .collect()
    }

    /// Copy into a SIBLING and rename it into place: a dot-name is never a library.
    fn ensure_one(&self, name: &str) -> Note {
        let at = self.home.join(name);
        let swept = self.drop_husks(name);
        if at.exists() {
            let note = self
                .add_seed_version(name)
                .unwrap_or_else(|| Note::new(name, "current", "here"));
            return if swept.is_empty() {
                note
            } else {
                Note {
                    note: format!("{}{swept}", note.note),
                    ..note
                }
            };
        }
        let partial = self.home.join(format!(".seeding-{}", stamp()));
        let done = (|| -> std::io::Result<Note> {
            fs::create_dir_all(&self.home)?;
            let from = self.seed_of(name);
            // A published seed installs under its own number; a bare folder is version 1.
            let first = partial.join(self.seed_version(name).to_string());
            if let Some(from) = from.filter(|f| f.exists()) {
                copy_tree(from, &first, &publishable)?;
                write_first_manifest(name, &first)?;
                fs::rename(&partial, &at)?;
                return Ok(Note::new(
                    name,
                    "seeded",
                    format!("copied from {}", from.display()),
                ));
            }
            fs::create_dir_all(&first)?;
            write_first_manifest(name, &first)?;
            fs::rename(&partial, &at)?;
            Ok(Note::new(name, "created", "empty"))
        })();
        done.unwrap_or_else(|e| {
            rm(&partial);
            Note::failed(name, &e)
        })
    }

    /// The version the seed of `name` is: its manifest's when it is a published one, else UNPINNED_VERSION.
    fn seed_version(&self, name: &str) -> u64 {
        self.seed_of(name)
            .and_then(|f| read_manifest(f))
            .map_or(UNPINNED_VERSION, |m| m.version)
    }

    /// An app update brings a newer published seed: added as one more frozen version, nothing overwritten.
    fn add_seed_version(&self, name: &str) -> Option<Note> {
        let from = self.seed_of(name)?;
        let manifest = read_manifest(from)?;
        let at = self.home.join(name);
        let held = version_numbers(&at);
        if held
            .iter()
            .any(|v| is_copy_of_seed(read_manifest(&at.join(v.to_string())).as_ref(), &manifest))
        {
            return None;
        }
        // Its number taken by another version (an admin's publish): the next free one, the seed's number kept as `seed`.
        let taken = held.contains(&manifest.version);
        let version = if taken {
            held.iter().max().copied().unwrap_or(0) + 1
        } else {
            manifest.version
        };
        let partial = self.home.join(format!(".seeding-{}", stamp()));
        let done = (|| -> std::io::Result<()> {
            copy_tree(from, &partial, &publishable)?;
            if taken {
                let renumbered = Manifest {
                    version,
                    seed: Some(manifest.version),
                    ..manifest.clone()
                };
                fs::write(partial.join(MANIFEST), stringify_manifest(&renumbered))?;
            }
            fs::rename(&partial, at.join(version.to_string()))
        })();
        Some(match done {
            Ok(()) if taken => Note::new(
                name,
                "seeded",
                format!(
                    "version {version} added from {}, whose version {} is another one here",
                    from.display(),
                    manifest.version
                ),
            ),
            Ok(()) => Note::new(
                name,
                "seeded",
                format!("version {version} added from {}", from.display()),
            ),
            Err(e) => {
                rm(&partial);
                Note::failed(name, &e)
            }
        })
    }

    /// Remove library `name`'s husks; answers what to say of them, empty when there were none.
    fn drop_husks(&self, name: &str) -> String {
        let at = self.home.join(name);
        let gone: Vec<u64> = version_numbers(&at)
            .into_iter()
            .filter(|v| is_husk(&at.join(v.to_string())))
            .collect();
        for v in &gone {
            rm(&at.join(v.to_string()));
        }
        if gone.is_empty() {
            return String::new();
        }
        let list: Vec<String> = gone.iter().map(u64::to_string).collect();
        format!(
            ", {} empty version folder(s) removed ({})",
            gone.len(),
            list.join(", ")
        )
    }

    /// Give library `name` the draft its admins edit, copied from its newest version; None when it has one.
    fn ensure_draft(&self, name: &str) -> Option<Note> {
        let at = self.home.join(name);
        let draft = at.join(DRAFT);
        if draft.exists() {
            return None;
        }
        let newest = version_numbers(&at).last().copied().unwrap_or(0);
        let partial = self.home.join(format!(".drafting-{}", stamp()));
        let skipped = at.join(newest.to_string()).join(MANIFEST);
        let done = (|| -> std::io::Result<()> {
            if newest > 0 {
                copy_tree(&at.join(newest.to_string()), &partial, &|src| {
                    src != skipped
                })?;
            } else {
                fs::create_dir_all(&partial)?;
            }
            fs::rename(&partial, &draft)
        })();
        Some(match done {
            Ok(()) => Note::new(
                name,
                "current",
                if newest > 0 {
                    format!("draft copied from version {newest}")
                } else {
                    "empty draft".into()
                },
            ),
            Err(e) => {
                rm(&partial);
                Note::failed(name, &e)
            }
        })
    }
}
