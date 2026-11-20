//! The account's Logs ring (2620): the last 1,000 lines, numbered from 1, a text cut at 16 KiB.
//! What a reader gets wrong: a cursor past `seq` answers from the start, as a restarted service's does in the TypeScript.

use std::collections::VecDeque;

use serde_json::{Value, json};

use super::{LINES, TEXT};

/// A line's level, spelled as the TypeScript spells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Info => "info",
            Level::Warn => "warn",
            Level::Error => "error",
        }
    }
    pub fn parse(s: &str) -> Option<Level> {
        match s {
            "info" => Some(Level::Info),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub seq: u64,
    pub t: u64,
    pub level: Level,
    pub text: String,
}

impl Line {
    /// The panel's shape, keys in the TypeScript's order.
    pub fn to_json(&self) -> Value {
        json!({ "seq": self.seq, "t": self.t, "level": self.level.as_str(), "text": self.text })
    }
}

#[derive(Default)]
pub struct Ring {
    lines: VecDeque<Line>,
    seq: u64,
}

/// `text` cut to at most `TEXT` bytes on a character boundary, ending ` …` when it was cut.
fn cut(text: &str) -> String {
    if text.len() <= TEXT {
        return text.to_string();
    }
    let mut end = TEXT;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{} …", &text[..end])
}

impl Ring {
    pub fn new() -> Ring {
        Ring::default()
    }

    /// The number of the last line pushed, 0 for none.
    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Keep one line, O(1); the oldest goes past 1,000.
    pub fn push(&mut self, level: Level, text: &str, t: u64) {
        self.seq += 1;
        self.lines.push_back(Line {
            seq: self.seq,
            t,
            level,
            text: cut(text),
        });
        if self.lines.len() > LINES {
            self.lines.pop_front();
        }
    }

    /// The cursor and the lines after `after`; with none, or one past `seq`, the last `tail`.
    pub fn since(&self, after: Option<f64>, tail: usize) -> (u64, Vec<Line>) {
        let past = after.is_none_or(|a| a > self.seq as f64);
        if past {
            let skip = self.lines.len().saturating_sub(tail);
            return (self.seq, self.lines.iter().skip(skip).cloned().collect());
        }
        let a = after.unwrap_or_default();
        if a.is_nan() {
            return (self.seq, Vec::new());
        }
        // Lines are consecutive, so the first one past the cursor is found by arithmetic.
        let first = self.seq - self.lines.len() as u64 + 1;
        let want = (a.floor() + 1.0).max(first as f64) as u64;
        let skip = (want - first) as usize;
        (self.seq, self.lines.iter().skip(skip).cloned().collect())
    }
}
