//! slots — the machine's SLOTS: frozen chains compiled once and kept here, one of which may be ENABLED, the
//! one the engine plays in its `slot` session with nobody attached. The store is files under the state folder,
//! each written beside and renamed over, so a crash leaves the last good one. What a reader gets wrong:
//! enabling here is only the memory of it; the part sends the document (`parts/slots`), and the engine drops
//! it when a project opens (`docs/slots.md`).

mod files;

pub use files::{Installed, Readable, bundle};

use std::path::Path;

use serde_json::Value;

use crate::paths::utf8;
use crate::protocol::SlotRow;

/// The engine session a slot plays in, the engine's `EngineHost::kSlotSession`.
pub const SESSION: &str = "slot";

const SLOT_FILE: &str = "slot.json";
const ENABLED_FILE: &str = "enabled";

/// Whether `id` may name a slot: a folder name on every platform, lowercase letters, digits and `-`, 64 at most.
pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

/// Written beside, then renamed over: a crash mid-write leaves the last whole file.
fn write_text(p: &Path, text: &str) -> bool {
    let Some(parent) = p.parent() else {
        return false;
    };
    let _ = std::fs::create_dir_all(parent);
    let mut tmp = p.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    if std::fs::write(&tmp, text).is_err() || std::fs::rename(&tmp, p).is_err() {
        let _ = std::fs::remove_file(&tmp);
        return false;
    }
    true
}

fn read_slot(dir: &Path, id: &str) -> Option<Value> {
    if !valid_id(id) {
        return None;
    }
    let text = std::fs::read(dir.join(id).join(SLOT_FILE)).ok()?;
    let j: Value = serde_json::from_slice(&text).ok()?;
    j.get("doc")?.is_object().then_some(j)
}

fn text(j: &Value, key: &str) -> String {
    j.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn footswitch_of(j: &Value) -> Option<i64> {
    j.get("footswitch")
        .and_then(Value::as_i64)
        .filter(|cc| (0..=127).contains(cc))
}

/// Every slot under `dir`, by id; one whose file does not read is left out.
pub fn list(dir: &Path) -> Vec<SlotRow> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        if !e.path().is_dir() {
            continue;
        }
        let id = e.file_name().to_string_lossy().into_owned();
        let Some(j) = read_slot(dir, &id) else {
            continue;
        };
        out.push(SlotRow {
            name: text(&j, "name"),
            chain: text(&j, "chain"),
            saved_at: text(&j, "savedAt"),
            protocol: j.get("protocol").and_then(Value::as_i64).unwrap_or(0),
            footswitch: footswitch_of(&j),
            id,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// The enabled slot's id, if one is enabled and still there.
pub fn enabled(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(ENABLED_FILE)).ok()?;
    let id = text.trim_end_matches(['\n', '\r', ' ']);
    // A mark naming a slot that is gone is no mark: the machine boots into nothing rather than an error.
    read_slot(dir, id).map(|_| id.to_string())
}

/// Remember `id` as the enabled one, or none.
pub fn set_enabled(dir: &Path, id: Option<&str>) {
    match id {
        None => {
            let _ = std::fs::remove_file(dir.join(ENABLED_FILE));
        }
        Some(id) => {
            write_text(&dir.join(ENABLED_FILE), &format!("{id}\n"));
        }
    }
}

/// Store the slot `staged` holds (`{id, name, chain, savedAt, protocol, doc}`), its files copied in (`files`),
/// replacing one of that id and keeping its footswitch; its id, or why not. The staged file is consumed.
pub fn save(dir: &Path, staged: &Path, installed: &Installed) -> Result<String, String> {
    let bytes =
        std::fs::read(staged).map_err(|_| format!("no slot to save at {}", utf8(staged)))?;
    let mut j: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    if !j.get("doc").is_some_and(Value::is_object) {
        return Err("a slot is {id, name, chain, savedAt, protocol, doc}, doc an object".into());
    }
    let id = text(&j, "id");
    if !valid_id(&id) {
        return Err("a slot's id is lowercase letters, digits and '-', 64 at most".into());
    }
    // The pedal is the machine's, not the chain's: saving the chain again keeps what it was bound to.
    match read_slot(dir, &id).as_ref().and_then(footswitch_of) {
        Some(cc) => j["footswitch"] = cc.into(),
        None => {
            j.as_object_mut().map(|o| o.remove("footswitch"));
        }
    }
    // Built whole beside, then swapped in: a refused or failed save leaves the slot it replaces as it was.
    let stage = dir.join(format!("{id}.new"));
    let _ = std::fs::remove_dir_all(&stage);
    let built = bundle(&mut j["doc"], &stage, &dir.join(&id), installed).and_then(|()| {
        write_text(&stage.join(SLOT_FILE), &j.to_string())
            .then_some(())
            .ok_or_else(|| format!("the slot could not be written under {}", utf8(dir)))
    });
    if let Err(e) = built {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(e);
    }
    let _ = std::fs::remove_dir_all(dir.join(&id));
    if let Err(e) = std::fs::rename(&stage, dir.join(&id)) {
        let _ = std::fs::remove_dir_all(&stage);
        return Err(format!("the slot could not be put in place: {e}"));
    }
    let _ = std::fs::remove_file(staged);
    Ok(id)
}

/// Forget slot `id`, and the enabled mark when it named it; false when there was none.
pub fn remove(dir: &Path, id: &str) -> bool {
    if !valid_id(id) || !dir.join(id).exists() {
        return false;
    }
    if enabled(dir).as_deref() == Some(id) {
        set_enabled(dir, None);
    }
    std::fs::remove_dir_all(dir.join(id)).is_ok()
}

/// Bind slot `id`'s footswitch to CC `cc` (0..127), or none; false when there is no such slot or no such CC.
pub fn set_footswitch(dir: &Path, id: &str, cc: Option<i64>) -> bool {
    let Some(mut j) = read_slot(dir, id) else {
        return false;
    };
    match cc {
        Some(cc) if !(0..=127).contains(&cc) => return false,
        Some(cc) => j["footswitch"] = cc.into(),
        None => {
            j.as_object_mut().map(|o| o.remove("footswitch"));
        }
    }
    write_text(&dir.join(id).join(SLOT_FILE), &j.to_string())
}

/// Slot `id`'s compiled document, as the engine's `project` message carries it, its footswitch added when it has one.
pub fn document(dir: &Path, id: &str) -> Option<Value> {
    let j = read_slot(dir, id)?;
    let mut doc = j["doc"].clone();
    // What a press bypasses is the chain's own plugins, which the compiler named.
    if let (Some(cc), Some(plugins)) = (
        footswitch_of(&j),
        j.get("chainPlugins").filter(|p| p.is_array()),
    ) {
        doc["footswitch"] = serde_json::json!({"cc": cc, "plugins": plugins});
    }
    Some(doc)
}
