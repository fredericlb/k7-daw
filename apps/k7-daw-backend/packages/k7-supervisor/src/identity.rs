//! identity.rs — the engine's stable identity, `<state>/engine-id.json`: an id, a token handed to the engine
//! and echoed on `whoami`, and a TUNNEL SECRET only the supervisor sends, to the registries it heartbeats
//! and tunnels to (`apps/k7-daw-backend/docs/engine/token.md`). The file is the owner's alone: 0600, in a 0700
//! folder when this creates it. A file from before the secret keeps its id and token and gains one.

use std::path::Path;

use serde_json::{Value, json};

use crate::settings::write_beside;

#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub id: String,
    pub token: String,
    /// Never logged, never echoed, never handed to the engine process.
    pub tunnel_secret: String,
    pub created_at: String,
}

/// `n` random bytes from the OS's CSPRNG.
fn random_bytes(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    getrandom::fill(&mut b).expect("the system's random source");
    b
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `n` random bytes as lowercase hex.
pub fn random_hex(n: usize) -> String {
    hex(&random_bytes(n))
}

/// A random RFC 4122 version-4 UUID.
pub fn random_uuid() -> String {
    let mut b = random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h = hex(&b);
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// Read and write for the owner alone; nothing to set where the filesystem has no modes.
fn owner_only(p: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
    }
    #[cfg(not(unix))]
    let _ = (p, mode);
}

/// The persisted identity, or one minted and written, so a machine keeps its id across restarts.
pub fn ensure(path: &Path) -> Identity {
    let read = std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let mut id = read.as_ref().and_then(|v| {
        Some(Identity {
            id: text(v, "id")?,
            token: text(v, "token")?,
            tunnel_secret: text(v, "tunnelSecret").unwrap_or_default(),
            created_at: text(v, "createdAt").unwrap_or_default(),
        })
    });
    if let Some(found) = &id
        && found.tunnel_secret.len() >= 32
    {
        owner_only(path, 0o600);
        return found.clone();
    }
    let mut minted = id.take().unwrap_or_else(|| Identity {
        id: random_uuid(),
        token: random_hex(24),
        tunnel_secret: String::new(),
        created_at: chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string(),
    });
    minted.tunnel_secret = random_hex(32);
    if let Some(dir) = path.parent()
        && !dir.exists()
    {
        let _ = std::fs::create_dir_all(dir);
        owner_only(dir, 0o700);
    }
    let body = json!({
        "id": minted.id, "token": minted.token, "tunnelSecret": minted.tunnel_secret, "createdAt": minted.created_at,
    });
    // A secret that is not on disk is one the next start replaces, and the registry then refuses that id.
    if write_beside(path, &format!("{body:#}\n")).is_err() {
        let _ = std::fs::write(path, format!("{body:#}\n"));
    }
    owner_only(path, 0o600);
    minted
}
