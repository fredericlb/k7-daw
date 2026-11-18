//! env.rs — the environment an account's file service starts with: the ALLOW-list,
//! taken from the hub's. What a reader gets wrong: a variable the file service starts reading must be added
//! here and to `ENV_KEYS`, or it is silently unset there.

use std::collections::BTreeMap;

/// Read by the file service: the end of its stdin is its stop, the only graceful one Windows has.
pub const STDIN_STOP_ENV: &str = "ELECTRON_SHELL_STDIN_STOP";

/// What the OS and the programs a file service runs need.
pub const SYSTEM: &[&str] = &[
    "PATH",
    "Path",
    "HOME",
    "USER",
    "LOGNAME",
    "SHELL",
    "TMPDIR",
    "TMP",
    "TEMP",
    "LANG",
    "LANGUAGE",
    "TZ",
    "NODE_ENV",
    "NODE_EXTRA_CA_CERTS",
    "NODE_COMPILE_CACHE",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "NO_PROXY",
    "http_proxy",
    "https_proxy",
    "no_proxy",
    "ELECTRON_RUN_AS_NODE",
    "ESBUILD_BINARY_PATH",
    "PREFIX",
    "LD_LIBRARY_PATH",
    "ANDROID_ROOT",
    "ANDROID_DATA",
    "SystemRoot",
    "SYSTEMROOT",
    "windir",
    "SystemDrive",
    "ComSpec",
    "PATHEXT",
    "APPDATA",
    "LOCALAPPDATA",
    "USERPROFILE",
    "HOMEDRIVE",
    "HOMEPATH",
    "USERNAME",
    "ProgramData",
    "ProgramFiles",
    "ProgramFiles(x86)",
    "NUMBER_OF_PROCESSORS",
    "PROCESSOR_ARCHITECTURE",
    "OS",
];

/// What the file service's code and the engine it runs read.
pub const STUDIO: &[&str] = &[
    "K7_LOOP_SITES",
    "STUDIO_CHROMIUM",
    "STUDIO_DEMOS",
    "STUDIO_ENGINE",
    "STUDIO_ENGINE_ROOTS",
    "STUDIO_FAUST_LIBS",
    "STUDIO_HUB_URL",
    "STUDIO_LIBRARIES_HOME",
    "STUDIO_MEDIATEK",
    "STUDIO_MODELS",
    "STUDIO_PARSE_CACHE",
    "STUDIO_RENDER_DEBUG",
    "STUDIO_RENDER_JOBS_URL",
    "STUDIO_RENDER_TRACE",
    "STUDIO_WHISPER_MODEL",
    "FFMPEG_BIN",
    "FFPROBE_BIN",
    "DIRT_SAMPLES",
    "STRUDEL_SAMPLES",
];

const PREFIXES: [&str; 4] = ["LC_", "XDG_", "TERMUX_", "PLAYWRIGHT_"];

/// Whether a file service may see variable `name`.
pub fn allowed(name: &str) -> bool {
    !name.is_empty()
        && (SYSTEM.contains(&name)
            || STUDIO.contains(&name)
            || PREFIXES.iter().any(|p| name.starts_with(p)))
}

/// The subset of `from` a file service may see.
pub fn service_env<'a>(
    from: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> BTreeMap<String, String> {
    from.into_iter()
        .filter(|(k, _)| allowed(k))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_what_the_service_reads_and_no_secret() {
        let parent = [
            ("PATH", "/bin"),
            ("STUDIO_ENGINE", "/e"),
            ("LC_ALL", "C"),
            ("SESSION_SECRET", "s"),
            ("OIDC_CLIENT_SECRET", "s"),
            ("STUDIO_SYNC_TOKEN", "s"),
            ("K7_TOKEN", "s"),
            ("STUDIO_RENDER_KEY", "s"),
        ];
        let out = service_env(parent);
        assert_eq!(
            out.keys().collect::<Vec<_>>(),
            ["LC_ALL", "PATH", "STUDIO_ENGINE"]
        );
    }

    #[test]
    fn lets_the_loop_sites_switch_through_to_an_account() {
        assert!(allowed("K7_LOOP_SITES"));
        let out = service_env([("K7_LOOP_SITES", "1"), ("K7_TOKEN", "s")]);
        assert_eq!(out.keys().collect::<Vec<_>>(), ["K7_LOOP_SITES"]);
    }
}
