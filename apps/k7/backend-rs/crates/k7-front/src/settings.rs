//! settings.rs — the TypeScript backend's own settings, read under the same names and defaults, so a
//! route answers the same in either language. A setting the front cannot know leaves its routes to the
//! backend (`claims_sessions`), never to a guess.

use std::collections::BTreeMap;
use std::path::PathBuf;

/// @studio/oidc's default, which `auth_guard` refuses outside a dev session or a device, and read only there.
const DEV_SECRET: &str = crate::auth_guard::DEFAULT_SESSION_SECRET;

#[derive(Clone, Debug)]
pub struct Settings {
    /// `SESSION_SECRET`; @studio/oidc's default only where the dev stub may run, else empty, and then the front claims no session.
    pub session_secret: String,
    /// A device's backend: one local user, and nobody but this machine answered.
    pub local_user: bool,
    pub prefs_dir: PathBuf,
    /// Where the session revocations live; None when the front cannot tell, and then owns no session route.
    pub auth_dir: Option<PathBuf>,
    /// The root domain's hosts, where the backend answers a page and not the API.
    pub site_hosts: Vec<String>,
    pub build: String,
    /// Proxies before the front whose `X-Forwarded-*` is believed.
    pub trust_hops: usize,
    /// `K7_FRONT_TRUSTED_PROXIES`: when set, only a socket peer in it is believed, and `trust_hops` is 1 (`peer::screen`).
    pub trusted_proxies: Option<Vec<crate::peer::Cidr>>,
    /// Where the apps' builds are kept: `STUDIO_APP_RELEASES_DIR`, else `~/.studio/app-releases`.
    pub app_releases_dir: PathBuf,
    /// The CI's upload bearer; None refuses every upload.
    pub app_release_token: Option<String>,
    /// One uploaded file's ceiling in bytes, `APP_RELEASE_MAX_MB` (2048 by default) as `Number()` reads it.
    pub app_release_max_bytes: f64,
    /// `K7_CAMERA_ICE_SERVERS`, as written: the STUN and TURN servers handed to both ends of a remote camera.
    pub camera_ice_servers: Option<String>,
    /// `SESSION_SECRET` as written, `""` when unset: what the file hub's ticket key derives from.
    pub raw_session_secret: String,
    /// The instance's data folder, `$STUDIO_MEDIATEK` or `<STUDIO_HOME or ~/.k7>/mediatek`.
    pub mediatek_dir: PathBuf,
    /// The library tokens' key, as `mediaTokens` derives it: `API_TOKEN_SECRET`, else `${SESSION_SECRET}:pl8`.
    pub api_token_secret: String,
    /// `K7_LOCAL_ENGINE=front`: on a device, the front runs the supervisor and the backend does not (`docs/local.md`).
    pub local_engine_front: bool,
    /// `K7_SUPERVISOR_BIN`, which a device's host names; None runs no local engine.
    pub supervisor_bin: Option<PathBuf>,
    /// The supervisor's `$STUDIO_HOME`, `K7_SUPERVISOR_HOME` or `~/.k7`.
    pub supervisor_home: PathBuf,
    /// The folders the engine opens by path: the files root and the media, as given.
    pub engine_roots: Vec<PathBuf>,
    /// The bearer the tray handed over at spawn, `K7_TRAY_TOKEN`.
    pub tray_token: Option<String>,
    /// `K7_RELEASE`, else the backend's package version: what `/api/local` names.
    pub release: String,
    /// Every setting the front was given, which the supervisor gets over this process's environment.
    pub supervisor_env: Vec<(String, String)>,
    /// Where the engines' bindings live: `STUDIO_ENGINES_DIR`, else `<mediatek>/.engines`.
    pub engines_dir: PathBuf,
    /// The tunnel's upload limits, as `uploadLimits` reads them.
    pub upload_limits: k7_engine::tunnel::uploads::UploadLimits,
    /// `K7_ENGINE_CORE=front`: the registry, the relay and the hub answered here, not by the backend (`docs/engine.md`).
    pub engine_core: bool,
    /// The front's own folder for compiled modules: `K7_FRONT_CACHE`, else `<STUDIO_HOME or ~/.k7>/front-cache`.
    pub front_cache: PathBuf,
    /// `K7_FILES=front`: the file hub answered here once the backend runs none of it (`docs/files.md`).
    pub files_front: bool,
    /// Where AI organize asks Claude: `ANTHROPIC_BASE_URL`, as the SDK reads it, or the API's own host.
    pub anthropic_base_url: String,
    /// `STUDIO_FILES_ROOT`: where the accounts' folders live.
    pub files_root: Option<PathBuf>,
    /// The volumes `/api/ready` probes, as `volumesFromEnv` names them.
    pub ready_volumes: Vec<String>,
    /// The built page, `K7_FRONTEND_DIST`; None looks for `apps/k7/frontend/dist` above the front.
    pub frontend_dist: Option<PathBuf>,
    /// `K7_DEV_FRONTEND`: a running Vite that serves the page in its place, as the dev tray asks.
    pub dev_frontend: Option<String>,
}

impl Settings {
    pub fn new(env: &BTreeMap<String, String>, trust_hops: usize) -> Self {
        let get = |k: &str| {
            env.get(k)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        // Untrimmed, as `env.X || default` reads them there.
        let raw = |k: &str| env.get(k).filter(|v| !v.is_empty()).cloned();
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_default();
        let mediatek = raw("STUDIO_MEDIATEK")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                raw("STUDIO_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".k7"))
                    .join("mediatek")
            });
        // An entry that is not one trusts nothing more: `Config::from_env` refuses it before.
        let proxies = get("K7_FRONT_TRUSTED_PROXIES")
            .map(|v| crate::peer::parse_proxies(&v).unwrap_or_default());
        let dev_stub = crate::auth_guard::dev_stub_allowed(env);
        let dev_secret = || {
            if dev_stub {
                DEV_SECRET.to_string()
            } else {
                String::new()
            }
        };
        let files_front = get("K7_FILES").as_deref() == Some("front");
        Settings {
            session_secret: get("SESSION_SECRET").unwrap_or_else(dev_secret),
            local_user: get("STUDIO_LOCAL_USER").as_deref() == Some("1"),
            prefs_dir: get("STUDIO_PREFS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".studio").join("prefs").join("k7")),
            auth_dir: get("STUDIO_AUTH_DIR")
                .map(PathBuf::from)
                .or_else(|| get("STUDIO_MEDIATEK").map(|m| PathBuf::from(m).join(".auth")))
                // `.auth` in the media's folder, where the session's revocations and the accounts' keys live.
                .or_else(|| Some(mediatek.join(".auth"))),
            site_hosts: get("STUDIO_SITE_HOSTS")
                .unwrap_or_else(|| "k7.rocks www.k7.rocks".into())
                .split([' ', ','])
                .filter(|h| !h.is_empty())
                .map(str::to_ascii_lowercase)
                .collect(),
            build: get("APP_VERSION").unwrap_or_else(|| "dev".into()),
            trust_hops: if proxies.is_some() { 1 } else { trust_hops },
            trusted_proxies: proxies,
            app_releases_dir: raw("STUDIO_APP_RELEASES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".studio").join("app-releases")),
            app_release_token: raw("APP_RELEASE_TOKEN"),
            app_release_max_bytes: get("APP_RELEASE_MAX_MB")
                .and_then(|v| v.parse::<f64>().ok())
                .filter(|n| *n != 0.0 && !n.is_nan())
                .unwrap_or(2048.0)
                * 1024.0
                * 1024.0,
            camera_ice_servers: get("K7_CAMERA_ICE_SERVERS"),
            raw_session_secret: env.get("SESSION_SECRET").cloned().unwrap_or_default(),
            engines_dir: raw("STUDIO_ENGINES_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| mediatek.join(".engines")),
            mediatek_dir: mediatek,
            engine_core: raw("K7_ENGINE_CORE").as_deref() == Some("front"),
            front_cache: raw("K7_FRONT_CACHE").map(PathBuf::from).unwrap_or_else(|| {
                raw("STUDIO_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".k7"))
                    .join("front-cache")
            }),
            files_front,
            frontend_dist: raw("K7_FRONTEND_DIST").map(PathBuf::from),
            dev_frontend: raw("K7_DEV_FRONTEND"),
            anthropic_base_url: raw("ANTHROPIC_BASE_URL")
                .unwrap_or_else(|| "https://api.anthropic.com".into()),
            files_root: raw("STUDIO_FILES_ROOT").map(|r| PathBuf::from(r.trim())),
            ready_volumes: ["STUDIO_FILES_ROOT", "STUDIO_MEDIATEK", "STUDIO_PREFS_DIR"]
                .iter()
                .filter_map(|k| raw(k))
                .fold(Vec::new(), |mut seen, d| {
                    if !seen.contains(&d) {
                        seen.push(d);
                    }
                    seen
                }),
            upload_limits: k7_engine::tunnel::uploads::UploadLimits::from_settings(&|k| raw(k)),
            api_token_secret: raw("API_TOKEN_SECRET").unwrap_or_else(|| {
                match raw("SESSION_SECRET") {
                    Some(s) => format!("{s}:pl8"),
                    // node's own key under the stub; anywhere else no key anyone could guess.
                    None if dev_stub => ":pl8".into(),
                    None => crate::routes::cameras::ticket::random_hex(32),
                }
            }),
            local_engine_front: get("K7_LOCAL_ENGINE").as_deref() == Some("front"),
            supervisor_bin: raw("K7_SUPERVISOR_BIN").map(PathBuf::from),
            supervisor_home: raw("K7_SUPERVISOR_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".k7")),
            engine_roots: ["STUDIO_FILES_ROOT", "STUDIO_MEDIATEK"]
                .iter()
                .filter_map(|k| raw(k).map(PathBuf::from))
                .collect(),
            tray_token: raw("K7_TRAY_TOKEN"),
            release: raw("K7_RELEASE").unwrap_or_else(|| env!("K7_VERSION").into()),
            supervisor_env: env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }

    /// Whether the front may answer a route behind the session: it must know where revocations are.
    pub fn claims_sessions(&self) -> bool {
        self.auth_dir.is_some() && !self.session_secret.is_empty()
    }
}
