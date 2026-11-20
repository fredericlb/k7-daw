//! The fake engine's answers to the three plugin queries: parameters, a window, a base written (2340, 2342).
//! What a reader gets wrong: no test opens a real window; `opened` is a word the fake says, and a saved base is a file of four bytes.

use serde_json::{Value, json};

/// What the fake says to a plugin query; the default finds everything and succeeds.
pub struct Script {
    pub found: bool,
    pub params: Vec<Value>,
    pub opened: bool,
    pub saved: bool,
    /// Another plugin's answer comes first, as when two surfaces ask at once.
    pub decoy: bool,
    pub quiet: bool,
}

impl Default for Script {
    fn default() -> Script {
        Script {
            found: true,
            params: vec![],
            opened: true,
            saved: true,
            decoy: false,
            quiet: false,
        }
    }
}

/// The frames a query frame gets, none for another kind of frame.
pub fn answers(script: &Script, frame: &Value) -> Vec<Value> {
    let kind = frame["type"].as_str().unwrap_or("");
    let (format, id) = (frame["format"].clone(), frame["id"].clone());
    let (real, decoy) = match kind {
        "list-plugin-params" => (
            json!({ "type": "plugin-param-list", "format": format, "id": id, "found": script.found, "params": script.params }),
            json!({ "type": "plugin-param-list", "format": "VST3", "id": "other", "found": true, "params": [] }),
        ),
        "show-native-plugin-ui-of" => (
            json!({ "type": "native-plugin-ui-opened", "format": format, "id": id, "ok": script.opened }),
            json!({ "type": "native-plugin-ui-opened", "format": "VST3", "id": "other", "ok": false }),
        ),
        "snapshot-plugin-base" => {
            let path = frame["path"].clone();
            if script.saved
                && let Some(p) = path.as_str()
            {
                let _ = std::fs::write(p, b"VST3");
            }
            (
                json!({ "type": "plugin-base-saved", "format": format, "id": id, "path": path, "ok": script.saved }),
                json!({ "type": "plugin-base-saved", "format": "VST3", "id": "other", "path": "other", "ok": false }),
            )
        }
        _ => return vec![],
    };
    if script.quiet {
        return vec![];
    }
    if script.decoy {
        vec![decoy, real]
    } else {
        vec![real]
    }
}
