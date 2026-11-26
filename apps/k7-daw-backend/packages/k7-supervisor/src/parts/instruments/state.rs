//! state.rs — the instruments part's state: what the machine keeps, the queue and the one fetch, the rows
//! and the downloads the snapshot carries, and the commands. The part holds its OWN copy of what is kept,
//! since the core's follows a tick behind a settings change it asked for.

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedSender;
use tokio::task::JoinHandle;

use super::Done;
use crate::core::Ctx;
use crate::instruments::{self, Instrument, Phase};
use crate::log;
use crate::protocol::{Command, Download, InstrumentRow};

pub(super) struct State {
    cx: Ctx,
    catalogue: Vec<Instrument>,
    root: PathBuf,
    idle: Duration,
    /// What this machine keeps; None until a first boot settles it.
    kept: Option<Vec<String>>,
    keeps_all: bool,
    queue: VecDeque<Instrument>,
    fetching: Option<String>,
    worker: Option<JoinHandle<()>>,
    removing: HashSet<String>,
    downloads: Vec<Download>,
    rows: Vec<InstrumentRow>,
    started: bool,
    done: UnboundedSender<Done>,
}

/// `$STUDIO_INSTRUMENTS=all` keeps every instrument, for a host with no Downloads tab to choose in.
pub(super) fn keeps_all_in(env: Option<&str>) -> bool {
    env == Some("all")
}

impl State {
    pub(super) fn new(
        cx: Ctx,
        catalogue: Vec<Instrument>,
        idle: Duration,
        done: UnboundedSender<Done>,
    ) -> State {
        State {
            root: cx.paths.instruments_dir(),
            cx,
            catalogue,
            idle,
            kept: None,
            keeps_all: keeps_all_in(std::env::var("STUDIO_INSTRUMENTS").ok().as_deref()),
            queue: VecDeque::new(),
            fetching: None,
            worker: None,
            removing: HashSet::new(),
            downloads: Vec::new(),
            rows: Vec::new(),
            started: false,
            done,
        }
    }

    fn wanted(&self, name: &str) -> bool {
        self.keeps_all
            || self
                .kept
                .as_ref()
                .is_some_and(|k| k.iter().any(|n| n == name))
    }

    fn find(&self, name: &str) -> Option<Instrument> {
        self.catalogue.iter().find(|i| i.name == name).cloned()
    }

    /// The rows and downloads the snapshot carries from now on.
    fn publish(&self) {
        let (rows, downloads) = (self.rows.clone(), self.downloads.clone());
        self.cx.patch(false, move |s| {
            s.instruments = rows.clone();
            s.downloads = downloads.clone();
        });
    }

    /// The Downloads tab's rows, re-read from disk when a choice, an install or a removal changes them.
    fn refresh(&mut self) {
        self.rows = self
            .catalogue
            .iter()
            .map(|i| InstrumentRow {
                name: i.name.clone(),
                label: i.label.clone(),
                bytes: i.bytes,
                wanted: self.wanted(&i.name),
                installed: instruments::installed(&self.root, i),
            })
            .collect();
    }

    /// Add or drop `name` from what this machine keeps, and remember it.
    fn keep(&mut self, name: &str, on: bool) {
        let mut list = self.kept.clone().unwrap_or_default();
        list.retain(|n| n != name);
        if on {
            list.push(name.to_string());
        }
        self.kept = Some(list.clone());
        self.cx.update_settings(move |s| s.instruments = Some(list));
    }

    /// A first boot keeps what this machine already has and fetches nothing new: the rest is the user's to ask for.
    pub(super) fn boot(&mut self) {
        self.kept = self.cx.settings.borrow().instruments.clone();
        if self.kept.is_none() && !self.keeps_all {
            let kept: Vec<String> = self
                .catalogue
                .iter()
                .filter(|i| instruments::present(&self.root, i))
                .map(|i| i.name.clone())
                .collect();
            log::host(&format!(
                "instruments: keeping the {} already on this machine",
                kept.len()
            ));
            let list = kept.clone();
            self.cx.update_settings(move |s| s.instruments = Some(list));
            self.kept = Some(kept);
        }
        self.refresh();
        self.publish();
    }

    /// The first engine of a run: queue every instrument this machine keeps and has not got, a newer pinned archive included.
    pub(super) fn engine_up(&mut self) {
        if std::mem::replace(&mut self.started, true) {
            return;
        }
        let missing: Vec<Instrument> = self
            .catalogue
            .iter()
            .filter(|i| self.wanted(&i.name) && !instruments::installed(&self.root, i))
            .cloned()
            .collect();
        for i in missing {
            self.enqueue(i);
        }
        self.publish();
        self.pump();
    }

    /// Queue `i` behind whatever is fetching; a failed attempt's row is replaced, one already queued is left alone.
    fn enqueue(&mut self, i: Instrument) {
        if self.fetching.as_deref() == Some(&i.name) || self.queue.iter().any(|q| q.name == i.name)
        {
            return;
        }
        self.downloads.retain(|d| d.name != i.name);
        self.downloads.push(Download {
            name: i.name.clone(),
            label: i.label.clone(),
            received: 0,
            total: i.bytes,
            phase: "queued".into(),
            error: String::new(),
        });
        self.queue.push_back(i);
    }

    /// Start the next queued fetch when none runs; the app is told as it goes.
    fn pump(&mut self) {
        if self.fetching.is_some() {
            return;
        }
        let Some(i) = self.queue.pop_front() else {
            return;
        };
        self.fetching = Some(i.name.clone());
        self.news(&i.name, 0, i.bytes, "downloading", "");
        let (root, idle, done) = (self.root.clone(), self.idle, self.done.clone());
        self.worker = Some(tokio::spawn(async move {
            log::host(&format!("instrument {}: downloading {}", i.name, i.url));
            // A frame per whole percent at most: each one redraws the app's status line.
            let mut last = (u64::MAX, Phase::Downloading);
            let news = done.clone();
            let name = i.name.clone();
            let result = instruments::install(&root, &i, idle, move |p| {
                let percent = (100 * p.received).checked_div(p.total).unwrap_or(0);
                if (percent, p.phase) != last {
                    last = (percent, p.phase);
                    let _ = news.send(Done::Progress(name.clone(), p));
                }
            })
            .await;
            let _ = done.send(Done::Fetched(i.name.clone(), result));
        }));
    }

    /// One download's news: an empty phase is a finished one, which leaves the list.
    fn news(&mut self, name: &str, received: u64, total: u64, phase: &str, error: &str) {
        let Some(d) = self.downloads.iter_mut().find(|d| d.name == name) else {
            return;
        };
        if phase.is_empty() {
            self.downloads.retain(|d| d.name != name);
        } else {
            d.phase = phase.into();
            d.error = error.into();
            if phase != "failed" {
                d.received = received;
                d.total = total;
            }
        }
        self.publish();
    }

    pub(super) fn done(&mut self, done: Done) {
        match done {
            Done::Progress(name, p) => {
                let phase = if p.phase == Phase::Unpacking {
                    "unpacking"
                } else {
                    "downloading"
                };
                self.news(&name, p.received, p.total, phase, "");
            }
            Done::Fetched(name, result) => {
                self.fetching = None;
                self.worker = None;
                match &result {
                    Ok(()) => log::host(&format!("instrument {name}: installed")),
                    Err(e) => log::host(&format!("instrument {name}: {e}")),
                }
                self.refresh();
                match result {
                    Ok(()) => self.news(&name, 0, 0, "", ""),
                    Err(e) => self.news(&name, 0, 0, "failed", &e),
                }
                self.pump();
            }
            Done::Removed { id, name, error } => {
                self.removing.remove(&name);
                log::host(&format!(
                    "instrument {name}{}",
                    if error.is_empty() {
                        ": removed".into()
                    } else {
                        format!(": {error}")
                    }
                ));
                self.refresh();
                self.publish();
                self.cx.reply(id, error.is_empty(), &error);
            }
        }
    }

    pub(super) fn command(&mut self, c: Command) {
        let Some(i) = self.find(&c.arg) else {
            let why = format!("{} is not an instrument this engine plays", c.arg);
            return self.cx.reply(c.id, false, &why);
        };
        if c.cmd == "instruments.download" {
            self.download(c.id, i);
        } else {
            self.remove(c.id, i);
        }
    }

    fn download(&mut self, id: i64, i: Instrument) {
        if self.removing.contains(&i.name) {
            return self
                .cx
                .reply(id, false, &format!("{} is still being deleted", i.label));
        }
        self.keep(&i.name, true);
        if !instruments::installed(&self.root, &i) {
            self.enqueue(i);
        }
        self.refresh();
        self.publish();
        self.pump();
        self.cx.reply(id, true, "");
    }

    fn remove(&mut self, id: i64, i: Instrument) {
        // A download cannot be cut mid-file: the worker would write into a folder being deleted.
        if self.fetching.as_deref() == Some(&i.name) {
            let why = format!("{} is downloading — remove it once it has landed", i.label);
            return self.cx.reply(id, false, &why);
        }
        self.keep(&i.name, false);
        self.queue.retain(|q| q.name != i.name);
        self.downloads.retain(|d| d.name != i.name);
        self.removing.insert(i.name.clone());
        self.refresh();
        self.publish();
        let (root, done) = (self.root.clone(), self.done.clone());
        tokio::task::spawn_blocking(move || {
            let error = instruments::remove(&root, &i).err().unwrap_or_default();
            let _ = done.send(Done::Removed {
                id,
                name: i.name,
                error,
            });
        });
    }

    /// The supervisor is going: a transfer outlives no supervisor, and a `.part` is what the next try resumes.
    pub(super) fn abort(&mut self) {
        if let Some(w) = self.worker.take() {
            w.abort();
        }
    }
}
