//! The link's `Host` made of the account's real parts (2216, 2230): the compile and the send-reference ports run on the shared song
//! sessions, the revision on the memo, the thawed freezes on the jobs' list, the preferences from the rig's own file.
//! What a reader gets wrong: a push's compile carries its own facts, a poll's reads them natively.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use k7_file_service::confine::Disk;
use k7_file_service::engine::docs::TakeArm;
use k7_file_service::engine::wire::Take;
use k7_file_service::libraries::Home;
use k7_file_service::revisions::Revisions;
use k7_file_service::rig::rig;
use k7_file_service::song_ask::{Input, song_roots};
use k7_file_service::space::Space;
use serde::Deserialize;
use serde_json::Value;
use song::doc::Vsts;
use song::graph::Folder;
use song::roots::Roots;
use song::session::{Ask, Session, View, engine_doc_of};
use song::{DiskSource, PathRule};

use super::super::jobs::Thawed;
use super::{Host, Prefs, held};
use crate::routes::files::service::Acct;

/// What a failed view's text starts with.
const FAILURE: &str = "{\"failure\":";

/// The facts of a compile beside its roots: read natively, or carried by a push.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Facts {
    #[serde(default)]
    rev: Option<String>,
    #[serde(default)]
    thawed: Vec<String>,
}

pub struct RealHost {
    rt: tokio::runtime::Handle,
    home: Home,
    /// The admin stamp while no jobs are held; with jobs, the jobs' `Thawed` holds the account's one flag.
    admin: std::sync::atomic::AtomicBool,
    thawed: OnceLock<Arc<Thawed>>,
    code_root: String,
    env: BTreeMap<String, String>,
    /// The account's native side: the song sessions the compiles read.
    acct: Arc<Acct>,
    /// Roots whose next compile ignores the revision token (a scan changed what the project was built against).
    fresh: Mutex<HashSet<String>>,
    /// Milliseconds a take's filing waits first, so a test has a slow filing that is real.
    #[cfg(test)]
    pub slow_ms: std::sync::atomic::AtomicU64,
}

/// What a project's session needs: its key, the roots, the rule and the disk.
struct Prepared {
    key: String,
    roots: Roots,
    rule: PathRule,
    disk: DiskSource,
}

impl RealHost {
    /// Must be made inside a runtime: a take's filing blocks on it.
    pub fn new(home: Home, code_root: String, acct: Arc<Acct>) -> Arc<RealHost> {
        Arc::new(RealHost {
            rt: tokio::runtime::Handle::current(),
            home,
            admin: false.into(),
            thawed: OnceLock::new(),
            code_root,
            env: acct.env.clone(),
            acct,
            fresh: Mutex::new(HashSet::new()),
            #[cfg(test)]
            slow_ms: 0.into(),
        })
    }

    /// The hub's stamp of the request in hand: who edits the libraries follows it.
    pub fn note_admin(&self, admin: bool) {
        match self.thawed.get() {
            Some(t) => t.note_admin(admin),
            None => self
                .admin
                .store(admin, std::sync::atomic::Ordering::Relaxed),
        }
    }

    /// The jobs' memory of the interactive asks: the admin stamp goes there (one flag per account).
    pub fn hold_thawed(&self, thawed: Arc<Thawed>) {
        let _ = self.thawed.set(thawed);
    }

    /// The account's home with the stamp of the last request.
    pub fn home_now(&self) -> Home {
        self.home()
    }

    fn home(&self) -> Home {
        let mut h = self.home.clone();
        h.admin = match self.thawed.get() {
            Some(t) => t.admin(),
            None => self.admin.load(std::sync::atomic::Ordering::Relaxed),
        };
        h
    }

    /// The account's native side the host reads the sessions from.
    pub fn acct(&self) -> Arc<Acct> {
        self.acct.clone()
    }

    fn prepared(&self, project: &str) -> Option<Prepared> {
        self.prepared_why(project).ok()
    }

    /// The project's roots and session key, or why it has none: the reason a refusal is worded from.
    fn prepared_why(&self, project: &str) -> Result<Prepared, &'static str> {
        let home = self.home();
        let input = Input {
            home: &home,
            project,
            code_root: &self.code_root,
            env: &self.env,
        };
        let mut roots = song_roots(&input, &Disk).map_err(|d| d.as_str())?;
        if roots.project.is_empty() {
            roots.project = project.to_string();
        }
        let ws = roots.confine.clone().unwrap_or_else(|| project.to_string());
        let rule = roots.paths;
        let key = k7_file_service::songs::key_of(&ws, rule).map_err(|_| "workspace")?;
        let (disk, _) = k7_file_service::songs::disk_of(&roots).map_err(|_| "mounts")?;
        Ok(Prepared {
            key,
            roots,
            rule,
            disk,
        })
    }

    /// `f` on a thread with the compiler's stack, as every native compile runs (1522): the blocking pool's 2 MiB would abort.
    fn on_stack<R: Send>(f: impl FnOnce() -> R + Send) -> Option<R> {
        std::thread::scope(|sc| {
            std::thread::Builder::new()
                .name("k7-front engine compile".into())
                .stack_size(crate::routes::files::service::native::stack::THREAD_STACK)
                .spawn_scoped(sc, f)
                .ok()?
                .join()
                .ok()
        })
    }

    /// The engine's document from the shared sessions, or the sentence of why there is none: a project that will not
    /// compile says what the compiler found, a decline says its reason (4-7).
    fn engine_doc(&self, root: &str, project: &str, facts: Facts) -> Result<Value, String> {
        let said = |why: &str| super::super::refuse::sentence(why).1;
        let p = self.prepared_why(project).map_err(said)?;
        let rig = rig(&self.home()).map_err(|d| said(d.as_str()))?;
        let fresh = held(&self.fresh).remove(root);
        // With the jobs held the thaw route is the front's, so the runtime's own list is never told of a thaw.
        let thawed = self.thawed.get().map_or(facts.thawed, |t| t.of(project));
        let ask = Ask {
            roots: p.roots,
            rig,
            vsts: Vsts::Scan,
            thawed,
            rev: if fresh { None } else { facts.rev },
            folder: Folder::Open,
        };
        let songs = self.acct.songs();
        let (key, rule, disk) = (p.key, p.rule, p.disk);
        let made =
            Self::on_stack(|| songs.with(&key, rule, |s| s.answer(&disk, &ask, &[View::Engine])))
                .ok_or_else(|| said("lost"))?;
        let reply = made
            .map_err(|_| said("retired"))?
            .map_err(|_| said("compile"))?;
        k7_file_service::songs::touch_used(&reply.renders_used);
        let text = reply
            .views
            .into_iter()
            .find(|(v, _)| *v == View::Engine)
            .ok_or_else(|| said("no-view"))?
            .1;
        if text.starts_with(FAILURE) {
            return Err(super::super::docs_failure::failure_message(&text));
        }
        let doc = engine_doc_of(&text).ok_or_else(|| said("engine-shape"))?;
        serde_json::from_str(doc).map_err(|_| said("engine-shape"))
    }

    /// A send-reference port over the project's session; `default` where the project does not read.
    fn with_refs<R: Send>(
        &self,
        project: &str,
        default: R,
        f: impl FnOnce(&mut Session, &Prepared) -> R + Send,
    ) -> R {
        let Some(p) = self.prepared(project) else {
            return default;
        };
        let songs = self.acct.songs();
        let key = p.key.clone();
        let rule = p.rule;
        let made = Self::on_stack(|| songs.with(&key, rule, |s| f(s, &p)));
        made.and_then(Result::ok).unwrap_or(default)
    }
}

impl Host for RealHost {
    fn compile(&self, root: &str, project: &str) -> Result<Value, String> {
        let facts = Facts {
            rev: Some(self.revision(root)).filter(|r| !r.is_empty()),
            thawed: Vec::new(),
        };
        self.engine_doc(root, project, facts)
    }

    fn compile_asked(
        &self,
        root: &str,
        project: &str,
        ask: &Value,
    ) -> Result<Option<Value>, String> {
        #[cfg(test)]
        std::thread::sleep(std::time::Duration::from_millis(
            self.slow_ms.load(std::sync::atomic::Ordering::Relaxed),
        ));
        let facts = serde_json::from_value::<Facts>(ask.clone()).unwrap_or_default();
        self.engine_doc(root, project, facts).map(Some)
    }

    fn invalidate(&self, root: &str) {
        held(&self.fresh).insert(root.to_string());
    }

    fn revision(&self, root: &str) -> String {
        let home = self.home();
        let space = Space::open(&home, root);
        match space.active() {
            Ok(dir) => Revisions::global().token(&home, space.root(), dir.as_deref(), false),
            Err(_) => String::new(),
        }
    }

    fn names_external(&self, _root: &str, project: &str) -> bool {
        self.with_refs(project, false, |s, p| {
            s.with_cache(p.rule, |cache| {
                song::send_refs::names_external_plugin(&p.disk, cache, &p.roots)
            })
        })
    }

    fn named_files(&self, root: &str, project: &str) -> Vec<String> {
        let records = self.with_refs(project, vec![], |s, p| {
            s.with_cache(p.rule, |cache| {
                song::send_refs::playlist_records(&p.disk, cache, &p.roots, root)
            })
        });
        let mut files = vec![];
        for rel in records {
            let abs = std::path::Path::new(root).join(&rel);
            if abs.is_file() {
                files.push(abs.display().to_string());
            }
            // A record's stems are `stems/` beside it, played in its place once separated.
            if let Some(dir) = abs.parent() {
                walk(&dir.join("stems"), &mut files);
            }
        }
        files.sort();
        files.dedup();
        files
    }

    fn open_projects(&self, files_root: &str) -> Vec<(String, String)> {
        let home = self.home();
        let Ok(markers) =
            std::fs::read_dir(std::path::Path::new(files_root).join(".active-projects"))
        else {
            return vec![];
        };
        let mut out = vec![];
        for e in markers.flatten() {
            let root = std::path::Path::new(files_root)
                .join(e.file_name())
                .display()
                .to_string();
            if let Ok(Some(project)) = Space::open(&home, &root).active() {
                out.push((root, project));
            }
        }
        out.sort();
        out
    }

    fn pattern_files(&self, project: &str) -> Vec<String> {
        self.with_refs(project, vec![], |s, p| {
            s.with_cache(p.rule, |cache| {
                song::send_refs::pattern_files(&p.disk, cache, &p.roots)
            })
        })
    }

    fn preferences(&self) -> Prefs {
        // The rig's own read, at each open; a file it cannot read leaves the defaults (5-4: legacy files are not upgraded).
        k7_file_service::rig::engine_prefs(&self.home())
            .map(|v| prefs_of(&v))
            .unwrap_or_default()
    }

    fn write_catalogue(&self, plugins: &Value) {
        let file = std::path::Path::new(&self.home.studio).join("exts.json");
        let _ = k7_hub::binding::write_scan_copy(plugins, &file);
    }

    fn file_take(
        &self,
        root: &str,
        project: &str,
        take: &Take,
        arm: Option<&TakeArm>,
    ) -> Result<String, String> {
        use super::filing::{Filing, file_stamp, file_take, meter_of};
        #[cfg(test)]
        std::thread::sleep(std::time::Duration::from_millis(
            self.slow_ms.load(std::sync::atomic::Ordering::Relaxed),
        ));
        // The tempo and metre are the compile's, which resolves what `project.k7x` alone would miss.
        let meter = meter_of(&self.compile(root, project)?)?;
        let home = self.home();
        let f = Filing {
            home: &home,
            rt: &self.rt,
            root,
            project,
        };
        file_take(&f, take, arm, meter, &file_stamp())
    }
}

/// Every file below `dir`, absolute; a folder the volume no longer has is nothing to bring.
fn walk(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(read) = std::fs::read_dir(dir) else {
        return;
    };
    for e in read.flatten() {
        let path = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&path, out),
            Ok(t) if t.is_file() => out.push(path.display().to_string()),
            _ => {}
        }
    }
}

fn prefs_of(v: &Value) -> Prefs {
    Prefs {
        audio_device: v.get("audioDevice").and_then(|d| {
            let name = d.get("name")?.as_str()?.to_string();
            let kind = d
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            Some((name, kind))
        }),
        input_pairs: v
            .get("inputPairs")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default(),
    }
}
