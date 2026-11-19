//! settings.rs — what the front reads of the environment a host hands it, under one name and one default each.

use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Settings {
    /// This install's token, the cookie a page presents (`docs/sessions.md`): `K7_SESSION_TOKEN`, else the auth folder's file, read at `build`.
    pub session_token: String,
    /// A device's backend: one local user, and nobody but this machine answered.
    pub local_user: bool,
    pub prefs_dir: PathBuf,
    /// Where the install's token lives.
    pub auth_dir: PathBuf,
    pub build: String,
    /// Proxies before the front whose `X-Forwarded-*` is believed.
    pub trust_hops: usize,
    /// `K7_CAMERA_ICE_SERVERS`, as written: the STUN and TURN servers handed to both ends of a remote camera.
    pub camera_ice_servers: Option<String>,
    /// The instance's data folder, `$STUDIO_MEDIATEK` or `<STUDIO_HOME or ~/.k7>/mediatek`.
    pub mediatek_dir: PathBuf,
    /// `K7_LOCAL_ENGINE=front`: on a device, the front runs the supervisor and the backend does not (`docs/local.md`).
    pub local_engine_front: bool,
    /// `K7_SUPERVISOR_BIN`, which a device's host names; None runs no local engine.
    pub supervisor_bin: Option<PathBuf>,
    /// The supervisor's `$STUDIO_HOME`, `K7_SUPERVISOR_HOME` or `~/.k7`.
    pub supervisor_home: PathBuf,
    /// The folders the engine opens by path: the files root and the media, as given.
    pub engine_roots: Vec<PathBuf>,
    /// `K7_RELEASE`, else the backend's package version: what `/api/local` names.
    pub release: String,
    /// Every setting the front was given, which the supervisor gets over this process's environment.
    pub supervisor_env: Vec<(String, String)>,
    /// Where the engines' bindings live: `STUDIO_ENGINES_DIR`, else `<mediatek>/.engines`.
    pub engines_dir: PathBuf,
    /// The tunnel's upload limits.
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
        let auth_dir = get("STUDIO_AUTH_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| mediatek.join(".auth"));
        let files_front = get("K7_FILES").as_deref() == Some("front");
        Settings {
            session_token: get("K7_SESSION_TOKEN").unwrap_or_default(),
            local_user: get("STUDIO_LOCAL_USER").as_deref() == Some("1"),
            prefs_dir: get("STUDIO_PREFS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".studio").join("prefs").join("k7")),
            auth_dir,
            build: env!("K7_VERSION").into(),
            trust_hops,
            camera_ice_servers: get("K7_CAMERA_ICE_SERVERS"),
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
            upload_limits: k7_engine::tunnel::uploads::UploadLimits::default(),
            local_engine_front: get("K7_LOCAL_ENGINE").as_deref() == Some("front"),
            supervisor_bin: raw("K7_SUPERVISOR_BIN").map(PathBuf::from),
            supervisor_home: raw("K7_SUPERVISOR_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join(".k7")),
            engine_roots: ["STUDIO_FILES_ROOT", "STUDIO_MEDIATEK"]
                .iter()
                .filter_map(|k| raw(k).map(PathBuf::from))
                .collect(),
            release: raw("K7_RELEASE").unwrap_or_else(|| env!("K7_VERSION").into()),
            supervisor_env: env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        }
    }
}
