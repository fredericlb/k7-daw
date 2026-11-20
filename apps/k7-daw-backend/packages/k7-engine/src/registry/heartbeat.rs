//! heartbeat.rs — reading an engine's heartbeat: only the id, the token and the
//! port can refuse one, a malformed tunnel secret too; every other odd field is CLEANED and the cleaning
//! said (`apps/k7-daw-backend/docs/engine/heartbeat.md`). The committed vectors hold the words.

use serde_json::{Map, Value, json};

use crate::js::{self, number_issues, string_issues};

/// A heartbeat the registry can keep, and what had to be cleaned out of it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReadHeartbeat {
    pub id: String,
    pub token: String,
    pub tunnel_secret: Option<String>,
    pub hostname: String,
    pub machine_fingerprint: String,
    pub loopback: String,
    pub lan_ips: Vec<String>,
    pub port: f64,
    /// The rating in the schema's key order, numbers as JavaScript holds them.
    pub performance: Option<Value>,
    pub library_level: Option<f64>,
    pub protocol: Option<f64>,
    pub build: Option<String>,
    pub safe_mode: bool,
    /// The chains this MACHINE separates with, by `want` — its own settings, not the caller's (`stemSplit.md`).
    pub chains: Option<Chains>,
    pub cleaned: Vec<String>,
}

/// An engine's own chains: which module and model it cuts with, and which it makes an instrumental with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Chains {
    pub stems: Option<String>,
    pub instrumental: Option<String>,
}

/// An engine id as a path segment and a log line can carry it.
fn is_engine_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// zod's `z.ipv4()`: four decimal parts, no leading zero, each at most 255.
pub fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 3
                && p.bytes().all(|b| b.is_ascii_digit())
                && (p.len() == 1 || !p.starts_with('0'))
                && p.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

/// `path: message` per issue, joined as `issuesOf` joins them.
fn issues_line(issues: &[(String, String)]) -> String {
    issues
        .iter()
        .map(|(p, m)| format!("{}: {m}", if p.is_empty() { "(body)" } else { p }))
        .collect::<Vec<_>>()
        .join("; ")
}

/// Every issue of `r` under `path`, its value kept when it had none.
fn collect<T>(
    issues: &mut Vec<(String, String)>,
    path: &str,
    r: Result<T, Vec<String>>,
) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(ms) => {
            issues.extend(ms.into_iter().map(|m| (path.to_string(), m)));
            None
        }
    }
}

/// `performanceSchema`: the rating in its key order, or its issues.
fn performance(v: &Value) -> Result<Value, String> {
    let Value::Object(p) = v else {
        return Err(issues_line(&[(
            String::new(),
            js::invalid_type("object", Some(v)),
        )]));
    };
    let mut issues = Vec::new();
    let mut out = Map::new();
    let fields: [(&str, bool, f64, f64, bool); 6] = [
        ("score", true, 0.0, 1_000_000.0, false),
        ("cores", true, 1.0, 4096.0, false),
        ("singleThread", false, 0.0, 1_000_000.0, false),
        ("diskWriteMBps", false, 0.0, 1_000_000.0, true),
        ("fsyncPerSec", false, 0.0, 10_000_000.0, true),
        ("memoryGB", false, 0.0, 1_000_000.0, false),
    ];
    for (key, int, min, max, nullable) in fields {
        let got = p.get(key);
        if nullable && got == Some(&Value::Null) {
            out.insert(key.into(), Value::Null);
            continue;
        }
        if let Some(n) = collect(&mut issues, key, number_issues(got, int, min, max)) {
            out.insert(key.into(), js::num(n));
        }
    }
    if issues.is_empty() {
        Ok(Value::Object(out))
    } else {
        Err(issues_line(&issues))
    }
}

/// A non-negative JavaScript integer from 1, as `Number.isInteger(x) && x >= 1` reads it.
fn level(v: Option<&Value>) -> Option<f64> {
    v.and_then(Value::as_f64)
        .filter(|n| n.fract() == 0.0 && *n >= 1.0)
}

/// Reads one heartbeat, or says which REQUIRED field made it unreadable.
pub fn read_heartbeat(body: &Value) -> Result<ReadHeartbeat, String> {
    let Value::Object(b) = body else {
        return Err(issues_line(&[(
            String::new(),
            js::invalid_type("object", Some(body)),
        )]));
    };
    let mut issues = Vec::new();
    let id_rule: &dyn Fn(&str) -> bool = &is_engine_id;
    let id = collect(
        &mut issues,
        "id",
        string_issues(
            b.get("id"),
            1,
            128,
            Some((id_rule, "an id is letters, digits, '.', '_' or '-'")),
        ),
    );
    let token = collect(
        &mut issues,
        "token",
        string_issues(b.get("token"), 1, 256, None),
    );
    let secret = match b.get("tunnelSecret") {
        None => None,
        got => collect(
            &mut issues,
            "tunnelSecret",
            string_issues(got, 32, 256, None),
        ),
    };
    let port = match b.get("addresses") {
        Some(Value::Object(a)) => collect(
            &mut issues,
            "addresses.port",
            number_issues(a.get("port"), true, 1.0, 65535.0),
        ),
        got => {
            issues.push(("addresses".into(), js::invalid_type("object", got)));
            None
        }
    };
    let (Some(id), Some(token), Some(port), true) = (id, token, port, issues.is_empty()) else {
        return Err(issues_line(&issues));
    };
    let empty = Map::new();
    let a = b
        .get("addresses")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    let mut cleaned = Vec::new();

    let loopback = match a.get("loopback") {
        Some(Value::String(s)) if is_ipv4(s) => s.clone(),
        None => "127.0.0.1".into(),
        Some(v) => {
            cleaned.push(format!(
                "addresses.loopback: {} is not IPv4, 127.0.0.1 used",
                js::stringify(v)
            ));
            "127.0.0.1".into()
        }
    };
    let mut lan_ips = Vec::new();
    match a.get("lanIps") {
        Some(Value::Array(ips)) => {
            for ip in ips {
                match ip {
                    Value::String(s) if is_ipv4(s) => lan_ips.push(s.clone()),
                    _ => cleaned.push(format!(
                        "addresses.lanIps: {} is not IPv4, left out",
                        js::stringify(ip)
                    )),
                }
            }
        }
        None => {}
        Some(_) => cleaned.push("addresses.lanIps: not a list, no LAN address kept".into()),
    }

    let performance = match b.get("performance") {
        None | Some(Value::Null) => None,
        Some(p) => match performance(p) {
            Ok(v) => Some(v),
            Err(e) => {
                cleaned.push(format!("performance left out — {e}"));
                None
            }
        },
    };
    let library_level = level(b.get("libraryLevel"));
    if library_level.is_none()
        && let Some(v) = b.get("libraryLevel")
    {
        cleaned.push(format!(
            "libraryLevel: {} is no level, left out",
            js::stringify(v)
        ));
    }
    let protocol = level(b.get("protocol"));
    if protocol.is_none()
        && let Some(v) = b.get("protocol")
    {
        cleaned.push(format!(
            "protocol: {} is no version, read as 1",
            js::stringify(v)
        ));
    }
    let build = match b.get("build") {
        Some(Value::String(s)) if is_stamp(s) => Some(s.clone()),
        None => None,
        Some(v) => {
            cleaned.push(format!("build: {} is no stamp, left out", js::stringify(v)));
            None
        }
    };
    let safe_mode = match b.get("safeMode") {
        None => false,
        Some(Value::Bool(m)) => *m,
        Some(v) => {
            cleaned.push(format!(
                "safeMode: {} is no flag, read as false",
                js::stringify(v)
            ));
            false
        }
    };
    // Read last, inside the answer, as the TypeScript's object literal reads them.
    let mut text = |key: &str| match b.get(key) {
        Some(Value::String(s)) => js::cut_utf16(s, 255).to_string(),
        None => String::new(),
        Some(_) => {
            cleaned.push(format!("{key}: not a string, left empty"));
            String::new()
        }
    };
    let hostname = text("hostname");
    let machine_fingerprint = text("machineFingerprint");
    // The machine's own chains: a name, trimmed; anything else is said and left out, as every odd field here is.
    let chains = match b.get("chains") {
        None | Some(Value::Null) => None,
        Some(Value::Object(o)) => {
            let mut one = |key: &str| match o.get(key) {
                Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_string()),
                Some(Value::String(_)) | None => None,
                Some(_) => {
                    cleaned.push(format!("chains.{key}: not a name, left out"));
                    None
                }
            };
            let chains = Chains {
                stems: one("stems"),
                instrumental: one("instrumental"),
            };
            (chains.stems.is_some() || chains.instrumental.is_some()).then_some(chains)
        }
        Some(_) => {
            cleaned.push("chains: not an object, left out".into());
            None
        }
    };
    Ok(ReadHeartbeat {
        id,
        token,
        tunnel_secret: secret.filter(|s| !s.is_empty()),
        hostname,
        machine_fingerprint,
        loopback,
        lan_ips,
        port,
        performance,
        library_level,
        protocol,
        build,
        safe_mode,
        chains,
        cleaned,
    })
}

/// `/^[A-Za-z0-9._-]{1,64}$/`.
fn is_stamp(s: &str) -> bool {
    (1..=64).contains(&s.len()) && is_engine_id(s)
}

impl ReadHeartbeat {
    /// The beat as `readHeartbeat` answers it, for the vectors.
    pub fn json(&self) -> Value {
        let mut o = Map::new();
        o.insert("id".into(), json!(self.id));
        o.insert("token".into(), json!(self.token));
        if let Some(s) = &self.tunnel_secret {
            o.insert("tunnelSecret".into(), json!(s));
        }
        o.insert("hostname".into(), json!(self.hostname));
        o.insert("machineFingerprint".into(), json!(self.machine_fingerprint));
        o.insert(
            "addresses".into(),
            json!({ "loopback": self.loopback, "lanIps": self.lan_ips, "port": js::num(self.port) }),
        );
        if let Some(p) = &self.performance {
            o.insert("performance".into(), p.clone());
        }
        if let Some(l) = self.library_level {
            o.insert("libraryLevel".into(), js::num(l));
        }
        if let Some(p) = self.protocol {
            o.insert("protocol".into(), js::num(p));
        }
        if let Some(b) = &self.build {
            o.insert("build".into(), json!(b));
        }
        if self.safe_mode {
            o.insert("safeMode".into(), json!(true));
        }
        if let Some(c) = &self.chains {
            let mut chains = Map::new();
            if let Some(s) = &c.stems {
                chains.insert("stems".into(), json!(s));
            }
            if let Some(s) = &c.instrumental {
                chains.insert("instrumental".into(), json!(s));
            }
            o.insert("chains".into(), Value::Object(chains));
        }
        o.insert("cleaned".into(), json!(self.cleaned));
        Value::Object(o)
    }
}
