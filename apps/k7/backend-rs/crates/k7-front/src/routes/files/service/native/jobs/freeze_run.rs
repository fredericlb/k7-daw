//! freeze_run.rs — the freezer's effects carried out: compiles, renders, reloads and the progress a share hears (2312 to 2316).
//! What a reader gets wrong: the machine (`k7_file_service::jobs::freezer`) decides and this only does, so no lock is held across a compile or a render.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Instant;

use crate::routes::files::service::native::callbacks::Reply;
use k7_file_service::jobs::AutoFreeze;
use k7_file_service::jobs::freezer::Freezer;
use k7_file_service::jobs::{FreezeEffect as Fx, FreezeEvent as Ev};
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::Jobs;
use super::render::render_ready;
use crate::routes::files::service::native::Home;
use crate::routes::files::service::native::docs::{self, Asked, Docs};
use crate::routes::files::service::native::engine::{EngineLink, Session};
use crate::routes::files::service::native::stack::{Failed, on_stack};
use song::session::View;

fn held<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One account's freezer: the machine, and what it needs to carry its effects out.
pub struct Freezing {
    jobs: Weak<Jobs>,
    link: EngineLink,
    docs: Docs,
    rt: Handle,
    machine: Mutex<Freezer>,
    /// The `renderAll` asks of a folder, answered in the order they were made.
    waiting: Mutex<HashMap<String, VecDeque<Reply>>>,
    /// A test's pause before a one-module list is written, to let a second thaw come while it waits.
    #[cfg(test)]
    pub(super) slow_apply: Mutex<Option<std::time::Duration>>,
    /// A test's compile, run where the real one runs: on the compile's own stack.
    #[cfg(test)]
    pub(super) fake: Mutex<Option<FakeCompile>>,
}

#[cfg(test)]
pub(super) type FakeCompile = Arc<dyn Fn(&str) -> Result<Vec<AutoFreeze>, String> + Send + Sync>;

impl Freezing {
    pub fn new(jobs: &Arc<Jobs>, link: EngineLink, docs: Docs, rt: Handle) -> Arc<Freezing> {
        let mut machine = Freezer::new();
        for (dir, list) in jobs.thawed.entries() {
            machine.set_thawed(&dir, &list);
        }
        Arc::new(Freezing {
            jobs: Arc::downgrade(jobs),
            link,
            docs,
            rt,
            machine: Mutex::new(machine),
            waiting: Mutex::new(HashMap::new()),
            #[cfg(test)]
            slow_apply: Mutex::new(None),
            #[cfg(test)]
            fake: Mutex::new(None),
        })
    }

    pub fn thaw(self: &Arc<Self>, dir: &str, module: &str, on: bool) {
        let (module, now) = (module.to_owned(), Instant::now());
        self.run(dir, Ev::Thaw { module, on, now });
    }

    #[cfg(test)]
    pub(super) fn running(&self, dir: &str) -> bool {
        held(&self.machine).running(dir)
    }

    #[cfg(test)]
    pub(super) fn reads_the_jobs_thawed_list(&self) -> bool {
        self.docs.thawed.is_some()
    }

    #[cfg(test)]
    pub(super) fn thawed_of(&self, dir: &str) -> Vec<String> {
        held(&self.machine).thawed(dir)
    }

    pub fn set_thawed(&self, dir: &str, modules: &[String]) {
        let mut machine = held(&self.machine);
        machine.set_thawed(dir, modules);
        self.apply_thawed(dir, modules.to_vec());
    }

    /// Written under the machine's guard, so the list the compile reads is the machine's last, never an older one.
    fn apply_thawed(&self, dir: &str, list: Vec<String>) {
        #[cfg(test)]
        let slow = *held(&self.slow_apply);
        #[cfg(test)]
        if let Some(wait) = slow.filter(|_| list.len() == 1) {
            std::thread::sleep(wait);
        }
        if let Some(j) = self.jobs.upgrade() {
            j.thawed.set(dir, list);
        }
    }

    /// `jobs.renderAll`: every missing render of `dir` now; the answer is the list no machine can make.
    pub fn render_all(self: &Arc<Self>, dir: &str, reply: Reply) {
        held(&self.waiting)
            .entry(dir.to_owned())
            .or_default()
            .push_back(reply);
        self.run(dir, Ev::RenderAll);
    }

    /// One pass over a served project: the timer's tick.
    pub fn tick(self: &Arc<Self>, session: &Arc<Session>) {
        let Some(served) = session.served().filter(|_| session.is_ready()) else {
            return;
        };
        let (me, playing) = (self.clone(), session.playing());
        let host = self.link.0.host.clone();
        self.rt.spawn(async move {
            let root = served.root.clone();
            let Ok(rev) = tokio::task::spawn_blocking(move || host.revision(&root)).await else {
                return;
            };
            let now = Instant::now();
            me.run(&served.project, Ev::Tick { rev, playing, now });
        });
    }

    fn run(self: &Arc<Self>, dir: &str, event: Ev) {
        let mut rest = Vec::new();
        let mut machine = held(&self.machine);
        for fx in machine.on(dir, event) {
            match fx {
                Fx::SetThawed(list) => self.apply_thawed(dir, list),
                other => rest.push(other),
            }
        }
        drop(machine);
        for fx in rest {
            self.effect(dir, fx);
        }
    }

    fn session_of(&self, dir: &str) -> Option<Arc<Session>> {
        let serves = |s: &&Arc<Session>| s.served().is_some_and(|v| v.project == dir);
        self.link.sessions().iter().find(serves).cloned()
    }

    fn effect(self: &Arc<Self>, dir: &str, fx: Fx) {
        let (me, dir) = (self.clone(), dir.to_owned());
        match fx {
            Fx::Compile => {
                self.rt.spawn(async move {
                    let done = me.compile(&dir).await;
                    let playing = me.session_of(&dir).is_some_and(|s| s.playing());
                    match done {
                        Ok(freezes) => me.run(&dir, Ev::Compiled { freezes, playing }),
                        Err(why) => me.run(&dir, Ev::CompileFailed(why)),
                    }
                });
            }
            Fx::Render(f) => {
                self.rt.spawn(async move {
                    let (made, error) = me.render(&f).await;
                    let module = f.module;
                    me.run(
                        &dir,
                        Ev::Rendered {
                            module,
                            made,
                            error,
                        },
                    );
                });
            }
            Fx::Reload => {
                if let Some(s) = self.session_of(&dir)
                    && let Some(v) = s.served()
                {
                    self.rt
                        .spawn(async move { s.reload(&v.root, &v.project).await });
                }
            }
            Fx::SetThawed(_) => {}
            Fx::Log(line) => self.link.0.tells.say(&line),
            Fx::Done(done) => {
                if let Some(j) = self.jobs.upgrade() {
                    let n = done.len() as u64;
                    j.counts
                        .freezes
                        .fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                }
            }
            Fx::Progress {
                done,
                total,
                module,
            } => {
                if let Some(j) = self.jobs.upgrade() {
                    j.emit(json!({ "kind": "jobs", "dir": dir, "done": done, "total": total, "module": module }));
                }
            }
            Fx::Impossible(list) => {
                let rows: Vec<Value> = list
                    .into_iter()
                    .map(|(module, plugins)| json!({ "module": module, "plugins": plugins }))
                    .collect();
                self.answer(&dir, Ok(json!({ "value": rows })));
            }
            Fx::RenderAllFailed(why) => self.answer(&dir, Err(why)),
        }
    }

    fn answer(&self, dir: &str, answer: Result<Value, String>) {
        let reply = held(&self.waiting)
            .get_mut(dir)
            .and_then(VecDeque::pop_front);
        if let Some(reply) = reply {
            reply(answer);
        }
    }

    /// The project compiled for its freezes, off the runtime on the compile's own stack (1522); a panic is a failed pass.
    pub(super) async fn compile(&self, dir: &str) -> Result<Vec<AutoFreeze>, String> {
        let host = self.link.0.host.clone();
        let (docs, project) = (self.docs.clone(), dir.to_owned());
        let rev = match self.session_of(dir).and_then(|s| s.served()) {
            Some(v) => tokio::task::spawn_blocking(move || host.revision(&v.root))
                .await
                .ok(),
            None => None,
        };
        #[cfg(test)]
        let fake = held(&self.fake).clone();
        let made = on_stack(move || {
            #[cfg(test)]
            if let Some(fake) = fake {
                return fake(&project).map(|f| Arc::from(json!({ "freezes": f }).to_string()));
            }
            freezes_text(&docs, &project, rev)
        })
        .await;
        let text = match made {
            Ok(Ok(t)) => t,
            Ok(Err(why)) => return Err(format!("the project could not be compiled ({why})")),
            Err(Failed::Panicked) => {
                if let Some(j) = self.jobs.upgrade() {
                    j.counts
                        .panics
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                return Err("the compile panicked".into());
            }
            Err(Failed::Lost) => return Err("the compile was lost".into()),
        };
        let mut freezes = parse_freezes(&text)?;
        // A render counts as made only when the stat said so; any other error is the pass's failure (2313).
        for f in &mut freezes {
            f.ready = render_ready(Path::new(&f.render))?;
        }
        Ok(freezes)
    }

    /// One freeze's render into its place; `made` is whether the file is there, whatever the error says.
    async fn render(&self, f: &AutoFreeze) -> (bool, Option<String>) {
        let out = PathBuf::from(&f.render);
        let error = match (
            out.parent().map(std::fs::create_dir_all),
            self.jobs.upgrade(),
        ) {
            (Some(Err(e)), _) => Some(e.to_string()),
            (_, None) => Some("the jobs are gone".to_owned()),
            (_, Some(j)) => match (&j.driver, serde_json::to_string(&f.cut)) {
                (Some(d), Ok(doc)) => {
                    j.counts
                        .renders
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    d.render_freeze(doc, out.clone()).await.err()
                }
                (None, _) => Some("no engine machine to render on".to_owned()),
                (_, Err(e)) => Some(e.to_string()),
            },
        };
        (out.exists(), error)
    }
}

/// The account's home as the last request stamped it, as `followAdmin` does (review B3 decision 2).
pub(super) fn home_of(docs: &Docs) -> Home {
    Home::of(&docs.env, docs.thawed.as_ref().is_some_and(|t| t.admin()))
}

/// The project's engine view compiled, whose `freezes` are the freezer's to read.
fn freezes_text(docs: &Docs, project: &str, rev: Option<String>) -> Result<Arc<str>, String> {
    let asked = Asked::of_project(project, rev);
    let p = docs::prepare_with(&home_of(docs), docs, asked).map_err(str::to_owned)?;
    docs::view_text_with(docs, true, &p, View::Engine).map_err(str::to_owned)
}

/// The renders the freezes of `project` would play, where a test caches them before a scenario.
#[cfg(test)]
pub(crate) fn renders_of(docs: &Docs, project: &str) -> Vec<String> {
    let text = freezes_text(docs, project, None).expect("the project compiles");
    let found = parse_freezes(&text).expect("the freezes parse");
    found.into_iter().map(|f| f.render).collect()
}

fn parse_freezes(text: &str) -> Result<Vec<AutoFreeze>, String> {
    let whole: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let list = whole.get("freezes").cloned().unwrap_or_else(|| json!([]));
    serde_json::from_value(list).map_err(|e| e.to_string())
}
