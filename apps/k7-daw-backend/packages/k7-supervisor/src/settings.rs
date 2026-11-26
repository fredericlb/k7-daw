//! settings.rs — the supervisor's persisted settings, `<state>/engine-host.json`: small and hand-edited, so
//! each field is read on its own and one bad key never loses the others. Run from `--config`, the file is
//! the user's and never written back. The core is the one writer: a part asks it (`core::Ctx::update_settings`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The k7 instances the user added, as their registries; overrides `$STUDIO_REGISTRY_URL`.
    pub registry_urls: Vec<String>,
    /// The engine's port: the first one tried, the next free one taken unless `port_fixed`.
    pub port: u16,
    /// `--port`: that port or none; never saved.
    pub port_fixed: bool,
    /// The interfaces in the user's order, best first.
    pub audio_priority: Vec<String>,
    /// The ONNX execution providers this machine separates on, lowercased; empty is the CPU.
    pub onnx_providers: Vec<String>,
    /// The instruments this machine keeps; None until a first boot settles it (`docs/instruments.md`).
    pub instruments: Option<Vec<String>>,
    /// The device token each studio paired this machine with, by registry URL.
    pub device_tokens: BTreeMap<String, String>,
    /// The engine listens on every interface and a device on this network drives it unpaired.
    pub lan_open: bool,
    /// The studios held a token for were listed once.
    pub pairings_listed: bool,
    /// The chain this machine separates with, and the one it makes an instrumental with.
    pub stems_chain: String,
    pub instrumental_chain: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            registry_urls: Vec::new(),
            port: 8765,
            port_fixed: false,
            audio_priority: Vec::new(),
            onnx_providers: Vec::new(),
            instruments: None,
            device_tokens: BTreeMap::new(),
            lan_open: false,
            pairings_listed: false,
            stems_chain: String::new(),
            instrumental_chain: String::new(),
        }
    }
}

/// The non-empty strings of a JSON array, first occurrence kept, in order.
fn unique_strings(v: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in v
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !s.is_empty() && !out.iter().any(|o| o == s) {
            out.push(s.to_string());
        }
    }
    out
}

fn trimmed(raw: &Map<String, Value>, key: &str) -> String {
    raw.get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string()
}

/// Lowercased, trimmed and deduplicated, since ORT's provider names are lowercase.
pub fn normalise_providers(v: Option<&Value>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in v.and_then(Value::as_array).into_iter().flatten() {
        let s = e.as_str().unwrap_or("").trim().to_lowercase();
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

impl Settings {
    /// A settings document read leniently; anything missing or malformed is its default.
    pub fn from_json(raw: &Value) -> Settings {
        let mut s = Settings::default();
        let Some(raw) = raw.as_object() else {
            return s;
        };
        s.registry_urls = unique_strings(raw.get("registryUrls"));
        if let Some(p) = raw
            .get("port")
            .and_then(Value::as_i64)
            .filter(|p| (1..65536).contains(p))
        {
            s.port = p as u16;
        }
        s.audio_priority = unique_strings(raw.get("audioPriority"));
        s.onnx_providers = normalise_providers(raw.get("onnxProviders"));
        // An array, even an empty one, is a choice; anything else is "never chosen".
        if raw.get("instruments").is_some_and(Value::is_array) {
            s.instruments = Some(unique_strings(raw.get("instruments")));
        }
        for (k, v) in raw
            .get("deviceTokens")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
        {
            if let Some(t) = v.as_str().map(str::trim).filter(|t| !t.is_empty()) {
                s.device_tokens.insert(k.clone(), t.to_string());
            }
        }
        // A config file's one `deviceToken` is the token of the studios that same file names.
        let one = trimmed(raw, "deviceToken");
        if !one.is_empty() {
            for u in &s.registry_urls {
                s.device_tokens
                    .entry(u.clone())
                    .or_insert_with(|| one.clone());
            }
        }
        s.lan_open = raw.get("lanOpen").and_then(Value::as_bool).unwrap_or(false);
        s.pairings_listed = raw
            .get("pairingsListed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        s.stems_chain = trimmed(raw, "stems");
        s.instrumental_chain = trimmed(raw, "instrumental");
        s
    }

    /// The document `from_json` reads back unchanged, in the file's established key order.
    pub fn to_json(&self) -> Value {
        let mut j = json!({
            "registryUrls": self.registry_urls,
            "port": self.port,
            "audioPriority": self.audio_priority,
            "onnxProviders": self.onnx_providers,
            "lanOpen": self.lan_open,
            "pairingsListed": self.pairings_listed,
        });
        let m = j.as_object_mut().expect("an object literal");
        if let Some(i) = &self.instruments {
            m.insert("instruments".into(), json!(i));
        }
        if !self.device_tokens.is_empty() {
            m.insert("deviceTokens".into(), json!(self.device_tokens));
        }
        // Written only when named: an absent key is the account's own setting.
        if !self.stems_chain.is_empty() {
            m.insert("stems".into(), json!(self.stems_chain));
        }
        if !self.instrumental_chain.is_empty() {
            m.insert("instrumental".into(), json!(self.instrumental_chain));
        }
        j
    }
}

/// The config file's keys the supervisor otherwise reads from the environment, as (variable, value).
pub fn config_environment(raw: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(m) = raw.as_object() else {
        return out;
    };
    for (key, var) in [
        ("machineName", "STUDIO_MACHINE_NAME"),
        ("extraCaCerts", "STUDIO_EXTRA_CA_CERTS"),
    ] {
        let v = trimmed(m, key);
        if !v.is_empty() {
            out.push((var.to_string(), v));
        }
    }
    // "all" keeps every instrument, as `$STUDIO_INSTRUMENTS=all` does.
    let all = m
        .get("instruments")
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(|e| e == "all"));
    if all {
        out.push(("STUDIO_INSTRUMENTS".into(), "all".into()));
    }
    out
}

/// Where the settings come from and go: `<state>/engine-host.json`, or a config file read only.
#[derive(Debug, Clone)]
pub struct Store {
    pub file: PathBuf,
    /// `--config`: never written back.
    pub read_only: bool,
    /// `--port`: every load has that port, fixed.
    pub fixed_port: Option<u16>,
}

impl Store {
    pub fn new(file: PathBuf) -> Store {
        Store {
            file,
            read_only: false,
            fixed_port: None,
        }
    }

    /// The file read; absent or unreadable is the defaults.
    pub fn load(&self) -> Settings {
        let raw = std::fs::read(&self.file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        let mut s = raw.map(|r| Settings::from_json(&r)).unwrap_or_default();
        if let Some(p) = self.fixed_port {
            s.port = p;
            s.port_fixed = true;
        }
        s
    }

    /// Written beside then renamed over; an unwritable home keeps the value for this session only.
    pub fn save(&self, s: &Settings) {
        if self.read_only {
            return;
        }
        let _ = write_beside(&self.file, &format!("{:#}\n", s.to_json()));
    }
}

/// `text` written to `<file>.tmp` and renamed over `file`: a crash mid-write leaves the last whole file.
pub fn write_beside(file: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = file.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, text)
        .and_then(|_| std::fs::rename(&tmp, file))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&tmp);
        })
}
