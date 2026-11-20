//! The background registry (2630 to 2632, 2614): one job per tag, listed under `tag_label(tag)` at once, held by a 10 minute
//! lease. What a reader gets wrong: the lease is renewed by what the job says, never by the clock, and a late progress is dropped.

use std::collections::HashMap;
use std::time::Instant;

use super::list::{Entry, State, Table};
use super::{LEASE, is_space};

struct Job {
    id: String,
    heard: Instant,
    lapsed_said: bool,
}

pub struct Jobs {
    table: Table,
    running: HashMap<String, Job>,
    origin: Instant,
    wall_ms: u64,
}

/// A background tag as a person reads it: the absolute path ending it shortened to its file name.
pub fn tag_label(tag: &str) -> String {
    let c: Vec<(usize, char)> = tag.char_indices().collect();
    let slash = |ch: char| ch == '/' || ch == '\\';
    let stops = |ch: char| matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    for (i, &(at, ch)) in c.iter().enumerate() {
        if !is_space(ch) {
            continue;
        }
        let drive = c.get(i + 1).is_some_and(|x| x.1.is_ascii_alphabetic())
            && c.get(i + 2).is_some_and(|x| x.1 == ':');
        let open = if drive { i + 3 } else { i + 1 };
        if !c.get(open).is_some_and(|x| slash(x.1)) {
            continue;
        }
        let mut last = None;
        for (k, &(_, x)) in c.iter().enumerate().skip(open + 1) {
            if stops(x) {
                break;
            }
            if slash(x) {
                last = Some(k);
            }
        }
        if let Some(k) = last {
            let end = c[k].0 + c[k].1.len_utf8();
            return format!("{} {}", &tag[..at], &tag[end..]);
        }
    }
    tag.to_string()
}

impl Jobs {
    /// `origin` is the instant of `wall_ms`, the epoch time the list's `startedAt` and `endedAt` count from.
    pub fn new(origin: Instant, wall_ms: u64) -> Jobs {
        Jobs {
            table: Table::new(),
            running: HashMap::new(),
            origin,
            wall_ms,
        }
    }

    fn ms(&self, now: Instant) -> u64 {
        self.wall_ms + now.saturating_duration_since(self.origin).as_millis() as u64
    }

    /// `bg.start`: false when the tag runs.
    pub fn start(&mut self, tag: &str, now: Instant) -> bool {
        let at = self.ms(now);
        self.table.prune(at);
        if self.running.contains_key(tag) {
            return false;
        }
        let id = self.table.start(&tag_label(tag), 0, at);
        self.running.insert(
            tag.to_string(),
            Job {
                id,
                heard: now,
                lapsed_said: false,
            },
        );
        true
    }

    /// Any message the runtime hears while a job is registered renews every lease.
    pub fn heard(&mut self, now: Instant) {
        for j in self.running.values_mut() {
            j.heard = now;
            j.lapsed_said = false;
        }
    }

    /// A progress tell: written on a running entry only, and a message heard.
    pub fn progress(
        &mut self,
        tag: &str,
        fraction: Option<f64>,
        detail: Option<&str>,
        now: Instant,
    ) {
        self.heard(now);
        if let Some(j) = self.running.get(tag) {
            self.table.progress(&j.id, fraction, detail);
        }
    }

    /// `bg.end`: the line the ring says, or none when the tag does not run (a second end).
    pub fn end(
        &mut self,
        tag: &str,
        outcome: Result<String, String>,
        now: Instant,
    ) -> Option<String> {
        let job = self.running.remove(tag)?;
        let at = self.ms(now);
        self.table.prune(at);
        Some(match outcome {
            Ok(line) => {
                self.table.end(&job.id, None, at);
                format!("   ✓ {line}")
            }
            Err(error) => {
                self.table.end(&job.id, Some(&error), at);
                format!("   ✗ {tag} failed: {error}")
            }
        })
    }

    /// Whether a job still holds the runtime busy and awake.
    pub fn holds(&self, now: Instant) -> bool {
        self.running
            .values()
            .any(|j| now.saturating_duration_since(j.heard) < LEASE)
    }

    /// The tags whose lease ran out since the last call, each once, in the order they started.
    pub fn lapsed(&mut self, now: Instant) -> Vec<String> {
        let mut out: Vec<(String, String)> = Vec::new();
        for (tag, j) in &mut self.running {
            if !j.lapsed_said && now.saturating_duration_since(j.heard) >= LEASE {
                j.lapsed_said = true;
                out.push((j.id.clone(), tag.clone()));
            }
        }
        out.sort_by_key(|(id, _)| id[1..].parse::<u64>().unwrap_or(0));
        out.into_iter().map(|(_, t)| t).collect()
    }

    /// The runtime is dropped under its jobs (2614): each ends failed, the lines to say.
    pub fn drop_all(&mut self, now: Instant) -> Vec<String> {
        let mut tags: Vec<(u64, String)> = self
            .running
            .iter()
            .map(|(t, j)| (j.id[1..].parse().unwrap_or(0), t.clone()))
            .collect();
        tags.sort();
        tags.into_iter()
            .filter_map(|(_, t)| self.end(&t, Err("the file service restarted".into()), now))
            .collect()
    }

    pub fn running(&self) -> usize {
        self.running.len()
    }

    /// The jobs for the process list, oldest first.
    pub fn list(&mut self, now: Instant) -> Vec<Entry> {
        let at = self.ms(now);
        self.table.list(at)
    }

    pub fn state_of(&self, tag: &str) -> Option<State> {
        self.running.get(tag).and_then(|j| self.table.state(&j.id))
    }
}
