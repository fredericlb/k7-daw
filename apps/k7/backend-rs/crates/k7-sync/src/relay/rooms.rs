//! rooms.rs — a relay's rooms loaded from their folders: a room's saved state with every entry since replayed, and the
//! live room of a slug and workspace made on first use.
//! What a reader gets wrong: a load runs outside the `rooms` map's lock, behind a gate of its own per room.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::{Inner, Live, lock};
use crate::fsx;
use crate::log::{self, ENTRIES, LogError};
use crate::room::Room;
use crate::settle::Js;

/// A room's state read from its folder: the saved state, then every entry appended since replayed; it WRITES the folder
/// where the log needs closing (a torn line, a legacy layout).
fn load(js: &dyn Js, dir: &Path) -> Result<Room, LogError> {
    let file = dir.join("room.json");
    let state = if file.exists() {
        let mut s = log::parse_state(&fsx::read_text(&file)?)?;
        // A retired op kind in the saved log is skipped, so a room saved before k7x still loads.
        log::without_retired_ops(&mut s)?;
        Some(s)
    } else {
        None
    };
    log::adopt_legacy(dir, state.as_ref())?;
    let mut room = state.as_ref().map(Room::from_state).unwrap_or_default();
    let entries = dir.join(ENTRIES);
    if entries.exists() {
        let text = fsx::read_text(&entries)?;
        // A torn last line is closed off, so the next append starts a line of its own instead of joining it.
        if !text.is_empty() && !text.ends_with('\n') {
            fsx::append(&entries, "\n")?;
        }
        for line in text.split('\n').filter(|l| !l.is_empty()) {
            if let Some(p) = log::parse_entry_line(line)? {
                room.replay(js, p.entry)?;
            }
        }
    }
    Ok(room)
}

impl Inner {
    /// The live room of `slug`'s `workspace`, loaded on first use.
    pub(super) fn live(&self, slug: &str, workspace: &str) -> Result<Arc<Mutex<Live>>, String> {
        let key = format!("{slug}/{workspace}");
        if let Some(l) = self.loaded(&key) {
            return Ok(l);
        }
        let gate = lock(&self.loading).entry(key.clone()).or_default().clone();
        let _one = lock(&gate);
        // Loaded by the caller this one waited on.
        if let Some(l) = self.loaded(&key) {
            return Ok(l);
        }
        let made = self.load_live(slug, workspace);
        if let Ok(l) = &made {
            lock(&self.rooms).insert(key.clone(), l.clone());
        }
        lock(&self.loading).remove(&key);
        made
    }

    pub(super) fn loaded(&self, key: &str) -> Option<Arc<Mutex<Live>>> {
        let l = lock(&self.rooms).get(key).cloned()?;
        lock(&l).used_at = Instant::now();
        Some(l)
    }

    pub(super) fn load_live(
        &self,
        slug: &str,
        workspace: &str,
    ) -> Result<Arc<Mutex<Live>>, String> {
        let dir = crate::path::room_dir(&(self.opts.account_dir)(slug), workspace);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let room = load(self.js.as_ref(), &dir).map_err(|e| e.message().to_string())?;
        Ok(Arc::new(Mutex::new(Live {
            room,
            dir,
            since_snapshot: 0.0,
            edited_at: None,
            used_at: Instant::now(),
        })))
    }
}
