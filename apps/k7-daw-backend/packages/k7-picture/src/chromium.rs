//! chromium.rs — the browser's launch: its flags, WebGL without a GPU, and the DevTools address read from its stderr.
//! What a reader gets wrong: the child gets the account's environment only (`env_clear`), never the front's own.

use std::path::Path;
use std::time::Duration;

/// chromiumoxide's own launch arguments, its headless mode's and ours; WebGL's are `webgl_args`'.
pub(crate) const CHROMIUM_ARGS: [&str; 29] = [
    "--disable-background-networking",
    "--enable-features=NetworkService,NetworkServiceInProcess",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-extensions",
    "--disable-features=TranslateUI",
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-renderer-backgrounding",
    "--disable-sync",
    "--force-color-profile=srgb",
    "--metrics-recording-only",
    "--no-first-run",
    "--enable-automation",
    "--password-store=basic",
    "--use-mock-keychain",
    "--enable-blink-features=IdleDetection",
    "--lang=en_US",
    "--headless",
    "--hide-scrollbars",
    "--mute-audio",
    // As Playwright starts it for node: the image runs chromium as an unprivileged user, where its sandbox cannot start.
    "--no-sandbox",
];

/// WebGL with no GPU: SwiftShader where the browser ships it (Playwright's, every OS), else Mesa's lavapipe through Vulkan (Alpine's).
pub(crate) fn webgl_args(chromium: &Path) -> [&'static str; 2] {
    let beside = std::fs::canonicalize(chromium)
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf));
    let swiftshader = ["libvk_swiftshader.so", "vk_swiftshader.dll"]
        .iter()
        .any(|f| beside.as_ref().is_some_and(|d| d.join(f).is_file()));
    if cfg!(target_os = "linux") && !swiftshader {
        // lavapipe is a CPU device, which chromium's GPU blocklist would refuse.
        ["--use-angle=vulkan", "--ignore-gpu-blocklist"]
    } else {
        ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"]
    }
}

/// How long chromium has to say where its DevTools listen, as chromiumoxide waits.
const LAUNCH_WAIT: Duration = Duration::from_secs(20);

/// chromium started here and not by chromiumoxide, which cannot clear what it inherits: its DevTools address, read from stderr.
pub(crate) async fn launch(
    chromium: &Path,
    env: &[(String, String)],
    profile: &Path,
) -> Result<(tokio::process::Child, String), String> {
    use tokio::io::AsyncBufReadExt;
    let mut cmd = tokio::process::Command::new(chromium);
    cmd.env_clear()
        .envs(env.iter().cloned())
        .args(CHROMIUM_ARGS)
        .args(webgl_args(chromium))
        .arg("--remote-debugging-port=0")
        .arg(format!("--user-data-dir={}", profile.display()))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("chromium did not start: {e}"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or("chromium did not start: no stderr")?;
    let mut lines = tokio::io::BufReader::new(stderr).lines();
    let found = tokio::time::timeout(LAUNCH_WAIT, async {
        while let Ok(Some(line)) = lines.next_line().await {
            if let Some((_, ws)) = line.rsplit_once("listening on ")
                && ws.starts_with("ws")
                && ws.contains("devtools/browser")
            {
                return Some(ws.trim().to_string());
            }
        }
        None
    })
    .await;
    match found {
        Ok(Some(ws)) => {
            // The rest of stderr read and dropped, so chromium never blocks on a full pipe.
            tokio::spawn(async move { while let Ok(Some(_)) = lines.next_line().await {} });
            Ok((child, ws))
        }
        Ok(None) => Err("chromium did not start: it exited before its DevTools listened".into()),
        Err(_) => Err("chromium did not start: no DevTools address within 20 s".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_chromium_without_its_sandbox_as_playwright_does() {
        assert!(CHROMIUM_ARGS.contains(&"--no-sandbox"));
    }

    /// Alpine's chromium ships no SwiftShader: its WebGL2 draws on Mesa's Vulkan, which needs the blocklist ignored.
    #[test]
    fn draws_on_swiftshader_where_the_browser_ships_it_else_on_vulkan() {
        let dir = std::env::temp_dir().join(format!("k7-picture-webgl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bin = dir.join("chromium");
        std::fs::write(&bin, "").unwrap();
        let bare = webgl_args(&bin);
        if cfg!(target_os = "linux") {
            assert_eq!(bare, ["--use-angle=vulkan", "--ignore-gpu-blocklist"]);
        } else {
            assert_eq!(
                bare,
                ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"]
            );
        }
        std::fs::write(dir.join("libvk_swiftshader.so"), "").unwrap();
        assert_eq!(
            webgl_args(&bin),
            ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// chromium sees the environment it is handed and nothing of this process's, a `HOME` included.
    #[cfg(unix)]
    #[tokio::test]
    async fn chromium_inherits_nothing() {
        use std::os::unix::fs::PermissionsExt;
        if std::env::var_os("HOME").is_none() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("k7-picture-env-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake = dir.join("chromium");
        let out = dir.join("env.txt");
        std::fs::write(&fake, "#!/bin/sh\nenv > \"$OUT\"\necho 'DevTools listening on ws://127.0.0.1:1/devtools/browser/x' >&2\nsleep 2\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let env = vec![("OUT".to_string(), out.display().to_string())];
        let (mut child, ws) = launch(&fake, &env, &dir.join("profile")).await.unwrap();
        assert_eq!(ws, "ws://127.0.0.1:1/devtools/browser/x");
        let _ = child.kill().await;
        let seen = std::fs::read_to_string(&out).unwrap();
        assert!(seen.lines().any(|l| l.starts_with("OUT=")), "{seen}");
        assert!(!seen.lines().any(|l| l.starts_with("HOME=")), "{seen}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
