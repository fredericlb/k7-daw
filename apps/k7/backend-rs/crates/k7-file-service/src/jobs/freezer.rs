//! freezer.rs — `AutoFreezer` of `autoFreezeJobs.ts` as a machine: per-folder state, events in, effects out (2312 to 2316).
//! What a reader gets wrong: `checked_rev` is set BEFORE the renders, so a failed render is not retried until the
//! project changes (2316), and a pass that ends starts the `RenderAll` queued behind it, as 2315's Notify does.

use super::{AutoFreeze, FreezeEffect as Fx, FreezeEvent as Ev, THAW_IDLE};
use crate::paths::resolve_all;
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

/// The modules left live in one folder, in the order thawed: the order 1o's compile key hashes (2310).
#[derive(Clone, Debug, Default)]
struct Thawed {
    order: Vec<String>,
    set: HashSet<String>,
}

impl Thawed {
    fn add(&mut self, m: &str) {
        if self.set.insert(m.to_owned()) {
            self.order.push(m.to_owned());
        }
    }

    fn remove(&mut self, m: &str) {
        if self.set.remove(m) {
            self.order.retain(|x| x != m);
        }
    }

    fn clear(&mut self) {
        self.order.clear();
        self.set.clear();
    }
}

struct Rendering {
    all: bool,
    queue: VecDeque<AutoFreeze>,
    current: Option<AutoFreeze>,
    done: Vec<String>,
    impossible: Vec<(String, Vec<String>)>,
    total: usize,
    started: usize,
}

enum Pass {
    Idle,
    // `stale`: a thaw landed while the compile ran, so its revision is not yet looked at.
    Compiling { all: bool, rev: String, stale: bool },
    Rendering(Box<Rendering>),
}

struct Dir {
    checked_rev: Option<String>,
    pending_reload: bool,
    thaw_activity: Option<Instant>,
    last_rev: Option<String>,
    applied: Option<String>,
    thawed: Thawed,
    pass: Pass,
    queued_all: usize,
}

impl Default for Dir {
    fn default() -> Dir {
        Dir {
            checked_rev: None,
            pending_reload: false,
            thaw_activity: None,
            last_rev: None,
            applied: None,
            thawed: Thawed::default(),
            pass: Pass::Idle,
            queued_all: 0,
        }
    }
}

/// Every served folder's freezer, keyed as node's `resolve(dir)` spells it.
#[derive(Default)]
pub struct Freezer {
    dirs: HashMap<String, Dir>,
}

fn plugin_names(f: &AutoFreeze) -> (String, Vec<String>) {
    (
        f.module.clone(),
        f.plugins.iter().map(|p| p.name.clone()).collect(),
    )
}

impl Freezer {
    pub fn new() -> Freezer {
        Freezer::default()
    }

    /// Whether a pass or a `renderAll` works on `dir` now.
    pub fn running(&self, dir: &str) -> bool {
        self.dirs
            .get(&resolve_all(&[dir]))
            .is_some_and(|d| !matches!(d.pass, Pass::Idle))
    }

    /// The modules left live in `dir`, in the order thawed.
    pub fn thawed(&self, dir: &str) -> Vec<String> {
        self.dirs
            .get(&resolve_all(&[dir]))
            .map(|d| d.thawed.order.clone())
            .unwrap_or_default()
    }

    /// `setThawed`: the list as given, duplicates dropped, the idle clock untouched.
    pub fn set_thawed(&mut self, dir: &str, modules: &[String]) {
        let d = self.dirs.entry(resolve_all(&[dir])).or_default();
        d.thawed.clear();
        for m in modules {
            d.thawed.add(m);
        }
    }

    /// One event of `dir`'s freezer; an event that fits no state of it (a stray answer) changes nothing.
    pub fn on(&mut self, dir: &str, e: Ev) -> Vec<Fx> {
        let d = self.dirs.entry(resolve_all(&[dir])).or_default();
        let mut fx = Vec::new();
        match e {
            Ev::Tick { rev, playing, now } => d.tick(rev, playing, now, &mut fx),
            Ev::Thaw { module, on, now } => d.thaw(&module, on, now, &mut fx),
            Ev::RenderAll => {
                d.queued_all += 1;
                d.start_queued(&mut fx);
            }
            Ev::Compiled { freezes, playing } => d.compiled(freezes, playing, &mut fx),
            Ev::CompileFailed(why) => d.failed(why, &mut fx),
            Ev::Rendered {
                module,
                made,
                error,
            } => d.rendered(&module, made, error, &mut fx),
        }
        fx
    }
}

impl Dir {
    fn idle(&self) -> bool {
        matches!(self.pass, Pass::Idle)
    }

    fn thaw(&mut self, module: &str, on: bool, now: Instant, fx: &mut Vec<Fx>) {
        if on {
            self.thawed.add(module);
        } else {
            self.thawed.remove(module);
        }
        fx.push(Fx::SetThawed(self.thawed.order.clone()));
        self.thaw_activity = Some(now);
        // What freezes has changed without any file doing so, and a compile in flight did not see it.
        self.checked_rev = None;
        if let Pass::Compiling { stale, .. } = &mut self.pass {
            *stale = true;
        }
    }

    fn tick(&mut self, rev: String, playing: bool, now: Instant, fx: &mut Vec<Fx>) {
        if !self.idle() || playing {
            fx.push(Fx::Done(Vec::new()));
            return;
        }
        if self.pending_reload {
            self.pending_reload = false;
            self.checked_rev = None;
            fx.push(Fx::Reload);
        }
        // An edit while thawed is the person still at it: the idle clock restarts.
        if self.last_rev.as_deref() != Some(rev.as_str()) {
            self.last_rev = Some(rev.clone());
            if !self.thawed.order.is_empty() {
                self.thaw_activity = Some(now);
            }
        }
        let idle_for = self.thaw_activity.map_or(std::time::Duration::ZERO, |at| {
            now.saturating_duration_since(at)
        });
        if !self.thawed.order.is_empty() && idle_for >= THAW_IDLE {
            self.thawed.clear();
            fx.push(Fx::SetThawed(Vec::new()));
            self.checked_rev = None;
            // Frozen again at once when the render for what is on disk already exists.
            fx.push(Fx::Reload);
        }
        if self.checked_rev.as_deref() == Some(rev.as_str()) {
            fx.push(Fx::Done(Vec::new()));
            return;
        }
        self.pass = Pass::Compiling {
            all: false,
            rev,
            stale: false,
        };
        fx.push(Fx::Compile);
    }

    fn start_queued(&mut self, fx: &mut Vec<Fx>) {
        if self.idle() && self.queued_all > 0 {
            self.queued_all -= 1;
            self.pass = Pass::Compiling {
                all: true,
                rev: String::new(),
                stale: false,
            };
            fx.push(Fx::Compile);
        }
    }

    fn compiled(&mut self, freezes: Vec<AutoFreeze>, playing: bool, fx: &mut Vec<Fx>) {
        let Pass::Compiling { all, rev, stale } = std::mem::replace(&mut self.pass, Pass::Idle)
        else {
            return;
        };
        let mut run = Rendering {
            all,
            queue: VecDeque::new(),
            current: None,
            done: Vec::new(),
            impossible: Vec::new(),
            total: 0,
            started: 0,
        };
        if all {
            let missing: Vec<&AutoFreeze> = freezes.iter().filter(|f| !f.ready).collect();
            run.impossible = missing
                .iter()
                .filter(|f| !f.hosted)
                .map(|f| plugin_names(f))
                .collect();
            run.queue = missing.into_iter().filter(|f| f.hosted).cloned().collect();
        } else {
            // A render made WITHOUT a VST would bake its stub: only an engine that has every VST renders it.
            run.queue = freezes
                .iter()
                .filter(|f| !f.ready && f.hosted)
                .cloned()
                .collect();
            self.checked_rev = (!stale).then_some(rev);
            self.reload_if_applied(playing, &freezes, fx);
        }
        run.total = run.queue.len();
        self.pass = Pass::Rendering(Box::new(run));
        self.next_render(fx);
    }

    /// Reload when the renders the compile plays changed: now if stopped, at the stop otherwise.
    fn reload_if_applied(&mut self, playing: bool, freezes: &[AutoFreeze], fx: &mut Vec<Fx>) {
        let mut renders: Vec<&str> = freezes
            .iter()
            .filter(|f| f.plays_render())
            .map(|f| f.render.as_str())
            .collect();
        // JavaScript's default sort compares UTF-16 code units, which is not the bytes of UTF-8.
        renders.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        let applied = renders.join("\n");
        let before = self.applied.replace(applied.clone());
        // The first look only learns: the document already sent was compiled from the same state.
        if before.is_none() || before.as_deref() == Some(applied.as_str()) {
            return;
        }
        if playing {
            self.pending_reload = true;
        } else {
            self.checked_rev = None;
            fx.push(Fx::Reload);
        }
    }

    fn next_render(&mut self, fx: &mut Vec<Fx>) {
        let Pass::Rendering(run) = &mut self.pass else {
            return;
        };
        if let Some(f) = run.queue.pop_front() {
            if run.all {
                fx.push(Fx::Progress {
                    done: run.started,
                    total: run.total,
                    module: f.module.clone(),
                });
            }
            run.started += 1;
            run.current = Some(f.clone());
            fx.push(Fx::Render(f));
            return;
        }
        let Pass::Rendering(run) = std::mem::replace(&mut self.pass, Pass::Idle) else {
            return;
        };
        if run.all {
            fx.push(Fx::Progress {
                done: run.total,
                total: run.total,
                module: String::new(),
            });
            fx.push(Fx::Impossible(run.impossible));
        } else {
            if !run.done.is_empty() {
                fx.push(Fx::Log(format!("   ❄ auto-froze {}", run.done.join(", "))));
            }
            fx.push(Fx::Done(run.done));
        }
        self.start_queued(fx);
    }

    fn rendered(&mut self, module: &str, made: bool, error: Option<String>, fx: &mut Vec<Fx>) {
        let Pass::Rendering(run) = &mut self.pass else {
            return;
        };
        let Some(f) = run.current.take_if(|f| f.module == module) else {
            return;
        };
        if let Some(why) = &error {
            fx.push(Fx::Log(format!("   ✗ auto-freeze {module}: {why}")));
        }
        if run.all {
            if !made {
                run.impossible.push(plugin_names(&f));
            }
        } else if error.is_none() && made {
            run.done.push(f.module);
        }
        self.next_render(fx);
    }

    fn failed(&mut self, why: String, fx: &mut Vec<Fx>) {
        let all = match std::mem::replace(&mut self.pass, Pass::Idle) {
            Pass::Idle => return,
            Pass::Compiling { all, .. } => all,
            Pass::Rendering(run) => run.all,
        };
        if all {
            fx.push(Fx::RenderAllFailed(why));
        } else {
            fx.push(Fx::Log(format!("   ✗ auto-freeze: {why}")));
            fx.push(Fx::Done(Vec::new()));
        }
        self.start_queued(fx);
    }
}
