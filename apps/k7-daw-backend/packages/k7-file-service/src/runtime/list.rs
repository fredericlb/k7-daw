//! The process table `listProcesses` keeps (2630): entries shown after a delay, kept `LINGER` after their end, in start order.
//! What a reader gets wrong: times are milliseconds on the caller's clock, and a process over before it was shown is forgotten.

use serde_json::{Value, json};

use super::LINGER;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub label: String,
    pub state: State,
    pub started_at: u64,
    pub ended_at: Option<u64>,
    pub progress: Option<f64>,
    pub detail: Option<String>,
    shown_at: u64,
}

impl Entry {
    /// The list's shape, a key present only when it holds a value.
    pub fn to_json(&self) -> Value {
        let state = match self.state {
            State::Running => "running",
            State::Done => "done",
            State::Failed => "failed",
        };
        let mut v = json!({ "id": self.id, "label": self.label, "state": state, "startedAt": self.started_at });
        let map = v.as_object_mut().unwrap();
        if let Some(p) = self.progress {
            // JSON.stringify prints 1, not 1.0.
            map.insert(
                "progress".into(),
                if p.fract() == 0.0 {
                    json!(p as i64)
                } else {
                    json!(p)
                },
            );
        }
        if let Some(d) = &self.detail {
            map.insert("detail".into(), json!(d));
        }
        if let Some(e) = self.ended_at {
            map.insert("endedAt".into(), json!(e));
        }
        v
    }
}

#[derive(Default)]
pub struct Table {
    entries: Vec<Entry>,
    counter: u64,
}

impl Table {
    pub fn new() -> Table {
        Table::default()
    }

    /// A process starts; its id is `p<n>`, numbered from 1 in this table.
    pub fn start(&mut self, label: &str, show_after_ms: u64, now: u64) -> String {
        self.counter += 1;
        let id = format!("p{}", self.counter);
        self.entries.push(Entry {
            id: id.clone(),
            label: label.to_string(),
            state: State::Running,
            started_at: now,
            ended_at: None,
            progress: None,
            detail: None,
            shown_at: now + show_after_ms,
        });
        id
    }

    /// What code inside a process says: a fraction clamped to 0..1 (None leaves it) and a word; written whatever the state.
    pub fn progress(&mut self, id: &str, fraction: Option<f64>, detail: Option<&str>) {
        let Some(e) = self.entries.iter_mut().find(|e| e.id == id) else {
            return;
        };
        if let Some(f) = fraction {
            e.progress = Some(f.clamp(0.0, 1.0));
        }
        if let Some(d) = detail {
            e.detail = Some(d.to_string());
        }
    }

    /// A process ends, failed with `error` or done; over before it was shown, it is forgotten.
    pub fn end(&mut self, id: &str, error: Option<&str>, now: u64) {
        let Some(at) = self.entries.iter().position(|e| e.id == id) else {
            return;
        };
        let e = &mut self.entries[at];
        e.state = if error.is_some() {
            State::Failed
        } else {
            State::Done
        };
        e.ended_at = Some(now);
        match error {
            Some(m) => e.detail = Some(m.to_string()),
            None => e.detail = None,
        }
        e.progress = None;
        if now < e.shown_at {
            self.entries.remove(at);
        }
    }

    pub fn state(&self, id: &str) -> Option<State> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.state)
    }

    /// Forget what ended longer ago than `LINGER`.
    pub fn prune(&mut self, now: u64) {
        let linger = LINGER.as_millis() as u64;
        self.entries
            .retain(|e| e.ended_at.is_none_or(|t| now.saturating_sub(t) <= linger));
    }

    /// What to show now, oldest first.
    pub fn list(&mut self, now: u64) -> Vec<Entry> {
        self.prune(now);
        self.entries
            .iter()
            .filter(|e| e.shown_at <= now)
            .cloned()
            .collect()
    }
}
