//! settings.rs — the one edit a host makes to the supervisor's `engine-host.json` before each start: this backend
//! first among the registries, every other loopback one dropped, the rest of the file the supervisor's;
//! the reasons are in `apps/k7-daw-backend/docs/localEngine.md`.

use std::path::Path;

use serde_json::{Map, Value};

/// `isLoopbackHost` of `@studio/web-protocol/services`: a URL's hostname, brackets and all for IPv6.
pub fn is_loopback_host(hostname: &str) -> bool {
    if hostname == "localhost" || hostname == "[::1]" {
        return true;
    }
    let Some(rest) = hostname.strip_prefix("127.") else {
        return false;
    };
    let parts: Vec<&str> = rest.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| (1..=3).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `u` names this machine; the supervisor reaches every such registry at one port, k7's (`Url.cpp`).
fn on_loopback(u: &str) -> bool {
    url::Url::parse(u)
        .ok()
        .and_then(|p| p.host_str().map(is_loopback_host))
        .unwrap_or(false)
}

/// `settings` with `local` as the first registry and no other on loopback; None when nothing changes.
pub fn pin_registry(settings: &Map<String, Value>, local: &str) -> Option<Map<String, Value>> {
    let was: Vec<&str> = match settings.get("registryUrls") {
        Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    let mut urls = vec![Value::String(local.to_string())];
    urls.extend(
        was.into_iter()
            .filter(|u| !on_loopback(u))
            .map(|u| Value::String(u.to_string())),
    );
    let urls = Value::Array(urls);
    if settings.get("registryUrls") == Some(&urls) {
        return None;
    }
    let mut pinned = settings.clone();
    pinned.insert("registryUrls".into(), urls);
    Some(pinned)
}

/// `engine-host.json` under `home` rewritten with `registry` pinned, beside then renamed; untouched when it already leads.
pub fn pin_file(home: &Path, registry: &str) -> std::io::Result<()> {
    let file = home.join("engine-host.json");
    // Unreadable or not an object: the supervisor would read it as its defaults, and so does the host.
    let kept = std::fs::read(&file)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default();
    let Some(pinned) = pin_registry(&kept, registry) else {
        return Ok(());
    };
    let text =
        serde_json::to_string_pretty(&Value::Object(pinned)).map_err(std::io::Error::other)?;
    let tmp = home.join("engine-host.json.tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, &file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const LOCAL: &str = "http://127.0.0.1:18789";

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().cloned().unwrap()
    }

    #[test]
    fn pins_this_backend_first_and_drops_other_loopback_spellings() {
        assert_eq!(
            pin_registry(&Map::new(), LOCAL).map(Value::Object),
            Some(json!({ "registryUrls": [LOCAL] }))
        );
        let s = obj(
            json!({ "port": 8765, "registryUrls": ["https://app.k7.rocks", "http://localhost:8789", "http://[::1]:1", "http://127.0.0.1:18789/"] }),
        );
        assert_eq!(
            Value::Object(pin_registry(&s, LOCAL).unwrap()).to_string(),
            r#"{"port":8765,"registryUrls":["http://127.0.0.1:18789","https://app.k7.rocks"]}"#
        );
        let led = obj(json!({ "registryUrls": [LOCAL, "https://app.k7.rocks"] }));
        assert!(pin_registry(&led, LOCAL).is_none());
    }

    #[test]
    fn loopback_hosts_are_named_as_web_protocol_names_them() {
        assert!(is_loopback_host("127.0.0.1") && is_loopback_host("localhost"));
        assert!(
            !is_loopback_host("127.0.0")
                && !is_loopback_host("localhost.")
                && !is_loopback_host("::1")
        );
    }
}
