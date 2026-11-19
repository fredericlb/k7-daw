//! config.rs — what the front is told: where it listens, the secret of its own seams, and how many proxies in
//! front of it are trusted to name the client.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// The address the front listens on.
    pub listen: SocketAddr,
    /// The secret of the front's own seams (the internal API, the engine slots); drawn per process when empty.
    #[serde(default)]
    pub secret: String,
    /// Proxies in front of this one whose `X-Forwarded-*` is believed, as Express's `trust proxy` counts them.
    #[serde(default)]
    pub trust_hops: usize,
    /// The settings the routes read (`ENV_KEYS`).
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// What the routes read of the environment a host hands the front.
pub const ENV_KEYS: [&str; 49] = [
    "K7_FRONT_TRUSTED_PROXIES",
    "K7_EXTRA_CA_CERTS",
    "SESSION_SECRET",
    "STUDIO_LOCAL_USER",
    "STUDIO_DEV_AUTH",
    "AUTH_ADMIN_GROUPS",
    "API_TOKEN_SECRET",
    "STUDIO_PREFS_DIR",
    "STUDIO_AUTH_DIR",
    "STUDIO_MEDIATEK",
    "APP_VERSION",
    "K7_CAMERA_ICE_SERVERS",
    "STUDIO_HOME",
    "K7_LOCAL_ENGINE",
    "K7_SUPERVISOR_BIN",
    "K7_SUPERVISOR_HOME",
    "K7_TRAY_TOKEN",
    "K7_RELEASE",
    "STUDIO_FILES_ROOT",
    "STUDIO_PORTS",
    "STUDIO_ENGINE",
    "STUDIO_RENDER_KEY",
    "STUDIO_REGISTRY_REQUIRE_DEVICE",
    "STUDIO_ENGINES_DIR",
    "STUDIO_TUNNEL_MAX_UPLOAD_BYTES",
    "STUDIO_TUNNEL_UPLOAD_BYTES_PER_HOUR",
    "K7_ENGINE_CORE",
    "K7_FRONT_CACHE",
    "K7_FILES",
    "K7_CODE_ROOT",
    "ANTHROPIC_BASE_URL",
    // The file hub's, and what it hands each account's runtime (`routes/files/hub`, `k7_hub::env`).
    "STUDIO_LIBRARY_SEED",
    "NODE_ENV",
    "NODE_EXTRA_CA_CERTS",
    "NODE_COMPILE_CACHE",
    "STUDIO_CHROMIUM",
    "STUDIO_DEMOS",
    "STUDIO_ENGINE_ROOTS",
    "STUDIO_MODELS",
    "STUDIO_PARSE_CACHE",
    "STUDIO_RENDER_DEBUG",
    "STUDIO_RENDER_TRACE",
    "STUDIO_WHISPER_MODEL",
    "FFMPEG_BIN",
    "FFPROBE_BIN",
    "DIRT_SAMPLES",
    "STRUDEL_SAMPLES",
    // The page and its fallback (`routes/last`).
    "K7_FRONTEND_DIST",
    "K7_DEV_FRONTEND",
];

impl Config {
    /// From `K7_FRONT_LISTEN`, `K7_FRONT_SECRET`, `K7_FRONT_TRUST_HOPS` and `ENV_KEYS`.
    pub fn from_env() -> Result<Self, String> {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// `from_env` over any lookup.
    pub fn from_vars(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let var = |name: &str| get(name).filter(|v| !v.is_empty());
        let listen = var("K7_FRONT_LISTEN").unwrap_or_else(|| "127.0.0.1:8789".into());
        if let Some(p) = var("K7_FRONT_TRUSTED_PROXIES") {
            crate::peer::parse_proxies(&p).map_err(|e| format!("K7_FRONT_TRUSTED_PROXIES: {e}"))?;
        }
        let config = Config {
            listen: listen
                .parse()
                .map_err(|e| format!("K7_FRONT_LISTEN {listen:?}: {e}"))?,
            secret: var("K7_FRONT_SECRET").unwrap_or_default(),
            trust_hops: match var("K7_FRONT_TRUST_HOPS") {
                Some(n) => n
                    .parse()
                    .map_err(|e| format!("K7_FRONT_TRUST_HOPS {n:?}: {e}"))?,
                None => 0,
            },
            env: ENV_KEYS
                .iter()
                .filter_map(|k| var(k).map(|v| (k.to_string(), v)))
                .collect(),
        };
        // A guessable session secret refuses the start before anything listens (`auth_guard`).
        if let Some(refused) = crate::auth_guard::refusal(&config.env) {
            return Err(refused);
        }
        config.settled()
    }

    /// The front's seams need a secret no client knows: one is drawn per process when none is named.
    pub fn settled(mut self) -> Result<Self, String> {
        if self.secret.is_empty() {
            let mut b = [0u8; 32];
            getrandom::fill(&mut b)
                .map_err(|e| format!("no randomness for the front's own secret: {e}"))?;
            self.secret = b.iter().map(|x| format!("{x:02x}")).collect();
        }
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::Config;

    fn from(pairs: &[(&str, &str)]) -> Result<Config, String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_vars(|k| map.get(k).cloned())
    }

    #[test]
    fn draws_its_own_secret_per_process_when_none_is_named() {
        let pairs = [("STUDIO_LOCAL_USER", "1")];
        let c = from(&pairs).expect("a front starts alone");
        assert_eq!(c.secret.len(), 64, "its own seams' secret is drawn");
        assert_ne!(
            from(&pairs).unwrap().secret,
            c.secret,
            "per process, never a constant"
        );
    }

    #[test]
    fn a_named_secret_is_kept() {
        let c = from(&[("STUDIO_LOCAL_USER", "1"), ("K7_FRONT_SECRET", "s")]).unwrap();
        assert_eq!(c.secret, "s");
    }

    #[test]
    fn a_guessable_session_secret_refuses_the_start_unless_a_device() {
        assert!(from(&[]).unwrap_err().contains("SESSION_SECRET"));
        assert!(from(&[("STUDIO_LOCAL_USER", "1")]).is_ok());
    }
}
