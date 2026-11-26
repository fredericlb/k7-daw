//! store.rs — the pairing store: `<state>/paired-devices.json` written whole on every change, the requests
//! waiting for the musician, and which live sockets each device speaks through. Port of Pairing.cpp's class.
//! What a reader gets wrong: the file is nlohmann's `dump(2)` byte for byte, because the C++ engine and this
//! one read each other's file (`fixtures/paired-devices.*.json` hold that format).

use super::SocketId;
use super::net::same_key;
use crate::log::say;
use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How long a request waits for the musician, and how many may wait at once: a LAN peer cannot queue the room.
const REQUEST_TTL_MS: i64 = 2 * 60 * 1000;
const MAX_PENDING: usize = 8;
/// "Last seen" is persisted at most this often: it is for a person deciding what to revoke, not a heartbeat.
const SEEN_SAVE_MS: i64 = 60 * 60 * 1000;
/// A stored key shorter than this is not a key the engine made.
const MIN_KEY_LEN: usize = 32;

#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub code: String,
    pub created_ms: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub created_ms: i64,
    pub last_seen_ms: i64,
}

/// An approval: the device made, its key (shown once), and the socket that asked.
#[derive(Debug)]
pub struct Approved {
    pub device: Device,
    pub key: String,
    pub from: Option<SocketId>,
}

struct Stored {
    device: Device,
    key: String,
}

#[derive(Default)]
struct Inner {
    devices: Vec<Stored>,
    requests: BTreeMap<String, (Request, Option<SocketId>)>,
    bound: HashMap<SocketId, String>,
}

pub struct Pairing {
    file: PathBuf,
    clock: fn() -> i64,
    inner: Mutex<Inner>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as i64)
}

/// `bytes` random bytes as hex, from the OS's generator: a key is a secret, not a hash seed.
fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the OS has no random generator");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// Six digits a person reads on one screen and finds on another.
fn random_code() -> String {
    let mut buf = [0u8; 4];
    getrandom::fill(&mut buf).expect("the OS has no random generator");
    format!("{:06}", u32::from_le_bytes(buf) % 1_000_000)
}

/// The first 64 bytes of `name`, a character cut in two printed as U+FFFD, as stored names already read.
fn clip_name(name: &str) -> String {
    if name.len() <= 64 {
        return name.to_string();
    }
    String::from_utf8_lossy(&name.as_bytes()[..64]).into_owned()
}

impl Pairing {
    /// The store at `file`, read now; a missing or unreadable file is no device paired.
    pub fn open(file: impl Into<PathBuf>) -> Self {
        Self::with_clock(file, now_ms)
    }

    /// The same with a clock of the caller's, for tests that move time.
    pub fn with_clock(file: impl Into<PathBuf>, clock: fn() -> i64) -> Self {
        let file = file.into();
        let devices = std::fs::read_to_string(&file)
            .ok()
            .and_then(|t| parse_store(&t))
            .unwrap_or_default();
        Pairing {
            file,
            clock,
            inner: Mutex::new(Inner {
                devices,
                ..Inner::default()
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A new request from `from`, replacing any it already had; None when too many are pending.
    pub fn request(&self, name: &str, ip: &str, from: Option<SocketId>) -> Option<Request> {
        let mut g = self.lock();
        let now = (self.clock)();
        prune(&mut g, now);
        // One pending request per socket: asking again replaces it, with a new code.
        if from.is_some() {
            g.requests.retain(|_, (_, s)| *s != from);
        }
        if g.requests.len() >= MAX_PENDING {
            return None;
        }
        let name = if name.is_empty() {
            "a device".to_string()
        } else {
            clip_name(name)
        };
        let r = Request {
            id: random_hex(4),
            name,
            ip: ip.to_string(),
            code: random_code(),
            created_ms: now,
        };
        g.requests.insert(r.id.clone(), (r.clone(), from));
        Some(r)
    }

    /// Approve: the device made, its key, and the socket that asked; None for a request gone or expired.
    pub fn approve(&self, request_id: &str) -> Option<Approved> {
        let mut g = self.lock();
        let now = (self.clock)();
        prune(&mut g, now);
        let (req, from) = g.requests.remove(request_id)?;
        let device = Device {
            id: random_hex(4),
            name: req.name,
            created_ms: now,
            last_seen_ms: now,
        };
        let key = random_hex(32);
        g.devices.push(Stored {
            device: device.clone(),
            key: key.clone(),
        });
        if let Some(s) = from {
            g.bound.insert(s, device.id.clone());
        }
        self.save(&g);
        Some(Approved { device, key, from })
    }

    /// Deny: the socket that asked, or None for a request gone.
    pub fn deny(&self, request_id: &str) -> Option<SocketId> {
        self.lock()
            .requests
            .remove(request_id)
            .and_then(|(_, from)| from)
    }

    /// The device holding `key`, now speaking through `on`; None for a key no device holds.
    pub fn verify(&self, key: &str, on: Option<SocketId>) -> Option<Device> {
        let mut g = self.lock();
        let now = (self.clock)();
        let at = g.devices.iter().position(|s| same_key(&s.key, key))?;
        let stale = now - g.devices[at].device.last_seen_ms > SEEN_SAVE_MS;
        g.devices[at].device.last_seen_ms = now;
        let device = g.devices[at].device.clone();
        if stale {
            self.save(&g);
        }
        if let Some(s) = on {
            g.bound.insert(s, device.id.clone());
        }
        Some(device)
    }

    /// Forget a device; the sockets it was speaking through are returned, to be untrusted and closed.
    pub fn revoke(&self, device_id: &str) -> Vec<SocketId> {
        let mut g = self.lock();
        let before = g.devices.len();
        g.devices.retain(|s| s.device.id != device_id);
        if g.devices.len() == before {
            return Vec::new();
        }
        let mut live: Vec<SocketId> = g
            .bound
            .iter()
            .filter(|(_, d)| *d == device_id)
            .map(|(s, _)| *s)
            .collect();
        live.sort_unstable();
        g.bound.retain(|_, d| d != device_id);
        self.save(&g);
        live
    }

    /// A socket closed: its pending request and its device binding go with it; true when a request went.
    pub fn forget(&self, ws: SocketId) -> bool {
        let mut g = self.lock();
        g.bound.remove(&ws);
        let before = g.requests.len();
        g.requests.retain(|_, (_, s)| *s != Some(ws));
        g.requests.len() != before
    }

    pub fn pending(&self) -> Vec<Request> {
        let mut g = self.lock();
        prune(&mut g, (self.clock)());
        g.requests.values().map(|(r, _)| r.clone()).collect()
    }

    pub fn devices(&self) -> Vec<Device> {
        self.lock()
            .devices
            .iter()
            .map(|s| s.device.clone())
            .collect()
    }

    /// The devices to disk, aside and renamed so a crash mid-write cannot unpair every device.
    fn save(&self, g: &Inner) {
        let text = store_text(&g.devices);
        let mut tmp = self.file.clone().into_os_string();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        if write_private(&tmp, text.as_bytes()).is_err() {
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &self.file) {
            say!("[pairing] cannot save {}: {e}", self.file.display());
        }
    }
}

fn prune(g: &mut Inner, now: i64) {
    g.requests
        .retain(|_, (r, _)| now - r.created_ms <= REQUEST_TTL_MS);
}

/// The keys are secrets: only the account the engine runs as may read the file.
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut opts, 0o600);
    let mut f = opts.open(path)?;
    #[cfg(unix)]
    std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
    f.write_all(bytes)
}

/// The store's devices from its text; None when it is not the store's format.
fn parse_store(text: &str) -> Option<Vec<Stored>> {
    let root: Value = serde_json::from_str(text).ok()?;
    let mut out = Vec::new();
    for d in root
        .as_object()?
        .get("devices")
        .map_or(Some(&Vec::new()), |v| v.as_array())?
    {
        let o = d.as_object()?;
        let text = |k: &str| -> Option<String> {
            o.get(k)
                .map_or(Some(String::new()), |v| v.as_str().map(String::from))
        };
        let num = |k: &str| -> Option<i64> {
            o.get(k).map_or(Some(0), |v| {
                v.as_i64().or_else(|| v.as_f64().map(|f| f as i64))
            })
        };
        let device = Device {
            id: text("id")?,
            name: text("name")?,
            created_ms: num("created")?,
            last_seen_ms: num("lastSeen")?,
        };
        out.push(Stored {
            device,
            key: text("key")?,
        });
    }
    out.retain(|s| s.key.len() >= MIN_KEY_LEN && !s.device.id.is_empty());
    Some(out)
}

/// The store's text: nlohmann's `dump(2)` of `{"devices": [...]}`, keys sorted, no trailing newline.
fn store_text(devices: &[Stored]) -> String {
    let list = devices
        .iter()
        .map(|s| {
            let mut m = Map::new();
            m.insert("id".into(), s.device.id.clone().into());
            m.insert("name".into(), s.device.name.clone().into());
            m.insert("key".into(), s.key.clone().into());
            m.insert("created".into(), s.device.created_ms.into());
            m.insert("lastSeen".into(), s.device.last_seen_ms.into());
            Value::Object(m)
        })
        .collect();
    let mut root = Map::new();
    root.insert("devices".into(), Value::Array(list));
    let mut out = String::new();
    pretty(&mut out, &Value::Object(root), 0);
    out
}

/// `dump(2)`: two spaces a level, `[]` and `{}` for the empty ones, leaves as the compact printer writes them.
fn pretty(out: &mut String, v: &Value, depth: usize) {
    let pad = |out: &mut String, d: usize| out.extend(std::iter::repeat_n(' ', d * 2));
    match v {
        Value::Array(items) if !items.is_empty() => {
            out.push_str("[\n");
            for (i, item) in items.iter().enumerate() {
                pad(out, depth + 1);
                pretty(out, item, depth + 1);
                out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
            }
            pad(out, depth);
            out.push(']');
        }
        Value::Object(map) if !map.is_empty() => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
            out.push_str("{\n");
            for (i, k) in keys.iter().enumerate() {
                pad(out, depth + 1);
                out.push_str(&crate::wire::json::dump(&Value::String((*k).clone())));
                out.push_str(": ");
                pretty(out, &map[k.as_str()], depth + 1);
                out.push_str(if i + 1 < keys.len() { ",\n" } else { "\n" });
            }
            pad(out, depth);
            out.push('}');
        }
        leaf => out.push_str(&crate::wire::json::dump(leaf)),
    }
}
