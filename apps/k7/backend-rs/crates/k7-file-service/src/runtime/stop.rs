//! The stop's plan (2670, 2671): members stopped one after the other, each optionally bounded, the whole optionally bounded,
//! and what had not stopped named when it gives up. What a reader gets wrong: the plan decides, the caller does the stopping.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Waiting,
    Running(Instant),
    Stopped,
    Hung,
}

/// What the caller does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Stop this member and say `finished` when it has.
    Run(String),
    /// Nothing to do before this instant, the earliest one the plan acts on.
    Wait(Instant),
    /// Over: every member stopped, or the bound passed.
    Done,
}

pub struct Stop {
    members: Vec<(String, Mark)>,
    each: Option<Duration>,
    deadline: Option<Instant>,
}

impl Stop {
    /// `members` in order; `each` bounds one member, `total` the whole from `now` (the reap sets neither bound of the whole).
    pub fn new(
        members: &[&str],
        each: Option<Duration>,
        total: Option<Duration>,
        now: Instant,
    ) -> Stop {
        Stop {
            members: members
                .iter()
                .map(|m| (m.to_string(), Mark::Waiting))
                .collect(),
            each,
            deadline: total.map(|t| now + t),
        }
    }

    /// The next thing to do at `now`.
    pub fn step(&mut self, now: Instant) -> Step {
        if self.expired(now) {
            return Step::Done;
        }
        let each = self.each;
        for (name, mark) in &mut self.members {
            match *mark {
                Mark::Stopped | Mark::Hung => continue,
                Mark::Waiting => {
                    *mark = Mark::Running(now);
                    return Step::Run(name.clone());
                }
                Mark::Running(since) => match each {
                    Some(e) if now.saturating_duration_since(since) >= e => *mark = Mark::Hung,
                    _ => return Step::Wait(self.wake(since)),
                },
            }
        }
        Step::Done
    }

    fn wake(&self, since: Instant) -> Instant {
        let own = self.each.map(|e| since + e);
        match (own, self.deadline) {
            (Some(a), Some(b)) => a.min(b),
            (a, b) => a
                .or(b)
                .unwrap_or(since + Duration::from_secs(60 * 60 * 24 * 365)),
        }
    }

    /// The caller stopped `member`; false when it was not the one running.
    pub fn finished(&mut self, member: &str) -> bool {
        for (name, mark) in &mut self.members {
            if name == member && matches!(mark, Mark::Running(_)) {
                *mark = Mark::Stopped;
                return true;
            }
        }
        false
    }

    pub fn expired(&self, now: Instant) -> bool {
        self.deadline.is_some_and(|d| now >= d)
    }

    /// Members that had not stopped: the hung ones and those never reached, in order.
    pub fn left(&self) -> Vec<&str> {
        let live = |m: &Mark| !matches!(m, Mark::Stopped);
        self.members
            .iter()
            .filter(|(_, m)| live(m))
            .map(|(n, _)| n.as_str())
            .collect()
    }
}

/// The one line the process says when it leaves with accounts unstopped, or none when all stopped.
pub fn leaving_line(accounts: &[(&str, &Stop)]) -> Option<String> {
    let parts: Vec<String> = accounts
        .iter()
        .filter(|(_, s)| !s.left().is_empty())
        .map(|(a, s)| format!("{a} ({})", s.left().join(", ")))
        .collect();
    (!parts.is_empty()).then(|| format!("stop: left with unstopped members: {}", parts.join("; ")))
}
