//! screens.rs — which other device shows which surface of the served project: the door's presence list.
//! A screen is forgotten when it says goodbye or stops beating for the TTL, which is short on purpose: a stale
//! "in use" locks the main device out of that pane. What a reader gets wrong: a screen of another project is dropped, not kept.

use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{Value, json};

/// A tablet that crashed is forgotten after this long without a beat.
pub const TTL_MS: u64 = 15_000;

struct Screen {
    surface: String,
    device: String,
    /// The served project it was announced for, `workspace/path`.
    project: String,
    since: u64,
    seen: u64,
}

#[derive(Default)]
pub struct Screens(Mutex<HashMap<String, Screen>>);

impl Screens {
    /// A beat from `screen`: first seen now, or kept as it was and seen again.
    pub fn beat(&self, id: &str, surface: &str, device: &str, project: &str, now: u64) {
        let mut map = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let since = map
            .get(id)
            .filter(|s| s.project == project && s.surface == surface)
            .map_or(now, |s| s.since);
        map.insert(
            id.to_string(),
            Screen {
                surface: surface.to_string(),
                device: device.to_string(),
                project: project.to_string(),
                since,
                seen: now,
            },
        );
    }

    pub fn count(&self) -> usize {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    pub fn bye(&self, id: &str) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).remove(id);
    }

    /// The live screens of `project` (none when nothing is served), oldest first; the lapsed and the other projects' are dropped.
    pub fn list(&self, project: Option<&str>, now: u64) -> Value {
        let mut map = self.0.lock().unwrap_or_else(|p| p.into_inner());
        map.retain(|_, s| {
            now.saturating_sub(s.seen) < TTL_MS && Some(s.project.as_str()) == project
        });
        let mut rows: Vec<&Screen> = map.values().collect();
        rows.sort_by_key(|s| (s.since, s.surface.clone()));
        Value::Array(
            rows.iter()
                .map(|s| json!({ "surface": s.surface, "device": s.device, "since": s.since }))
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_screen_lapses_after_the_ttl_and_a_beat_keeps_it() {
        let s = Screens::default();
        s.beat("a", "desk", "iPad", "ws/p", 1_000);
        assert_eq!(
            s.list(Some("ws/p"), 1_000 + TTL_MS - 1)
                .as_array()
                .unwrap()
                .len(),
            1
        );
        s.beat("a", "desk", "iPad", "ws/p", 10_000);
        assert_eq!(
            s.list(Some("ws/p"), 10_000 + TTL_MS - 1)
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(
            s.list(Some("ws/p"), 10_000 + TTL_MS)
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn bye_removes_at_once_and_the_first_sight_is_kept() {
        let s = Screens::default();
        s.beat("a", "desk", "iPad", "ws/p", 1_000);
        s.beat("a", "desk", "iPad", "ws/p", 3_000);
        assert_eq!(s.list(Some("ws/p"), 3_000)[0]["since"], 1_000);
        s.bye("a");
        assert!(s.list(Some("ws/p"), 3_000).as_array().unwrap().is_empty());
    }

    #[test]
    fn a_screen_of_another_project_is_dropped() {
        let s = Screens::default();
        s.beat("a", "desk", "iPad", "ws/old", 1_000);
        assert!(s.list(Some("ws/new"), 1_000).as_array().unwrap().is_empty());
        s.beat("b", "desk", "iPad", "ws/new", 1_000);
        assert!(s.list(None, 1_000).as_array().unwrap().is_empty());
    }
}
