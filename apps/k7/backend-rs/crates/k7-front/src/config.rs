//! config.rs — what the front is told: where it listens, the TypeScript backend behind it, the secret
//! that backend checks, and how many proxies in front of it are trusted to name the client.

use std::collections::BTreeMap;
use std::net::SocketAddr;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    /// The address the front listens on.
    pub listen: SocketAddr,
    /// The TypeScript backend, `host:port`, spoken to in plain HTTP/1.1; empty when the front runs alone.
    #[serde(default)]
    pub upstream: String,
    /// The secret the backend's `K7_FRONT_SECRET` holds, and the front's own seams'; drawn per process when it runs alone.
    #[serde(default)]
    pub secret: String,
    /// Proxies in front of this one whose `X-Forwarded-*` is believed, as Express's `trust proxy` counts them.
    #[serde(default)]
    pub trust_hops: usize,
    /// The backend's own settings the ported routes read (`ENV_KEYS`), as the TypeScript backend is given them.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// What the ported routes read of the TypeScript backend's environment, under the same names.
pub const ENV_KEYS: [&str; 58] = [
    "K7_FRONT_TRUSTED_PROXIES",
    "K7_EXTRA_CA_CERTS",
    "SESSION_SECRET",
    "STUDIO_LOCAL_USER",
    "STUDIO_DEV_AUTH",
    "AUTH_ADMIN_GROUPS",
    "OIDC_ISSUER",
    "OIDC_CLIENT_ID",
    "API_TOKEN_SECRET",
    "STUDIO_PREFS_DIR",
    "STUDIO_AUTH_DIR",
    "STUDIO_MEDIATEK",
    "STUDIO_SITE_HOSTS",
    "APP_VERSION",
    "STUDIO_APP_RELEASES_DIR",
    "APP_RELEASE_TOKEN",
    "APP_RELEASE_MAX_MB",
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
    "STUDIO_FAUST_LIBS",
    "STUDIO_MODELS",
    "STUDIO_PARSE_CACHE",
    "STUDIO_RENDER_DEBUG",
    "STUDIO_RENDER_TRACE",
    "STUDIO_WHISPER_MODEL",
    "FFMPEG_BIN",
    "FFPROBE_BIN",
    "DIRT_SAMPLES",
    "STRUDEL_SAMPLES",
    // The backend's last routes (`routes/last`).
    "K7_LAST_ROUTES",
    "K7_FRONTEND_DIST",
    "K7_DEV_FRONTEND",
];

impl Config {
    /// From `K7_FRONT_LISTEN`, `K7_FRONT_UPSTREAM`, `K7_FRONT_SECRET`, `K7_FRONT_TRUST_HOPS` and `ENV_KEYS`.
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
            // No backend unless one is named, once the front answers every route itself; node's port else.
            upstream: var("K7_FRONT_UPSTREAM").unwrap_or_else(|| {
                if answers_all(&|k| var(k)) {
                    String::new()
                } else {
                    "127.0.0.1:8790".into()
                }
            }),
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
        // node's own start-up refusal, before anything listens (`auth_guard`).
        if let Some(refused) = crate::auth_guard::refusal(&config.env) {
            return Err(refused);
        }
        config.settled()
    }

    /// A backend named needs its secret (fail-closed); no backend needs every route answered here, and draws its own seams' secret.
    pub fn settled(mut self) -> Result<Self, String> {
        if !self.upstream.is_empty() {
            if self.secret.is_empty() {
                return Err("K7_FRONT_SECRET is not set: the backend could not tell a client from this front".into());
            }
            return Ok(self);
        }
        let env = self.env.clone();
        if !answers_all(&|k| {
            env.get(k)
                .cloned()
                .or_else(|| std::env::var(k).ok())
                .filter(|v| !v.is_empty())
        }) {
            return Err("no backend is named (K7_FRONT_UPSTREAM), and K7_LAST_ROUTES, K7_FILES and K7_ENGINE_CORE are not all front: nothing would answer the backend's routes".into());
        }
        if self.secret.is_empty() {
            let mut b = [0u8; 32];
            getrandom::fill(&mut b)
                .map_err(|e| format!("no randomness for the front's own secret: {e}"))?;
            self.secret = b.iter().map(|x| format!("{x:02x}")).collect();
        }
        Ok(self)
    }

    /// Whether a backend is behind this front.
    pub fn has_backend(&self) -> bool {
        !self.upstream.is_empty()
    }
}

/// Whether the front answers every route itself: the last routes, the file hub and the engine core all `front`.
fn answers_all(get: &dyn Fn(&str) -> Option<String>) -> bool {
    ["K7_LAST_ROUTES", "K7_FILES", "K7_ENGINE_CORE"]
        .iter()
        .all(|k| get(k).as_deref() == Some("front"))
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

    const ALONE: [(&str, &str); 4] = [
        ("K7_LAST_ROUTES", "front"),
        ("K7_FILES", "front"),
        ("K7_ENGINE_CORE", "front"),
        ("STUDIO_LOCAL_USER", "1"),
    ];

    #[test]
    fn runs_alone_with_no_backend_and_no_secret_once_it_answers_every_route() {
        let c = from(&ALONE).expect("a front alone starts");
        assert!(!c.has_backend());
        assert_eq!(c.secret.len(), 64, "its own seams' secret is drawn");
        assert_ne!(
            from(&ALONE).unwrap().secret,
            c.secret,
            "per process, never a constant"
        );
    }

    #[test]
    fn a_named_backend_still_needs_the_secret() {
        let mut pairs = ALONE.to_vec();
        pairs.push(("K7_FRONT_UPSTREAM", "127.0.0.1:8790"));
        assert!(from(&pairs).unwrap_err().contains("K7_FRONT_SECRET"));
        pairs.push(("K7_FRONT_SECRET", "s"));
        assert_eq!(from(&pairs).unwrap().upstream, "127.0.0.1:8790");
    }

    #[test]
    fn without_every_switch_the_backend_is_node_s_and_its_secret_required() {
        let some = [
            ("K7_FILES", "front"),
            ("K7_ENGINE_CORE", "front"),
            ("STUDIO_LOCAL_USER", "1"),
        ];
        assert!(from(&some).unwrap_err().contains("K7_FRONT_SECRET"));
        let c = Config {
            upstream: String::new(),
            ..from(&[("K7_FRONT_SECRET", "s"), ("STUDIO_LOCAL_USER", "1")]).unwrap()
        };
        assert!(c.settled().unwrap_err().contains("no backend is named"));
    }
}
