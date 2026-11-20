// handlers.rs — what the window's webview asks the host: where a link may lead, what a new window or a download
// becomes, and which permissions the page has. The page is k7's own, on 127.0.0.1; everything else goes to the
// system browser. See docs/desktop.md.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::webview::{DownloadEvent, NewWindowResponse, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Manager, Url, WebviewWindowBuilder, Wry};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};

use crate::open;
use crate::state::SharedState;

type Builder<'a> = WebviewWindowBuilder<'a, Wry, AppHandle>;

/// Where a link leads.
#[derive(Debug, PartialEq)]
pub enum Route {
    /// The page stays where it is: k7 itself, the splash, a blob or a data URL.
    Stay,
    /// The system browser takes it.
    Browser,
    /// Nothing takes it.
    Drop,
}

pub fn route(url: &Url, port: u16) -> Route {
    match url.scheme() {
        "blob" | "data" | "about" | "tauri" => Route::Stay,
        "http" | "https" if url.host_str() == Some("tauri.localhost") => Route::Stay,
        "http" if is_page(url, port) => Route::Stay,
        "http" | "https" | "mailto" => Route::Browser,
        _ => Route::Drop,
    }
}

/// Whether `url` is on k7's own origin.
fn is_page(url: &Url, port: u16) -> bool {
    url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.port_or_known_default() == Some(port)
}

/// What the page may ask the webview for, on k7's origin only (docs/desktop.md).
pub fn permit(kind: PermissionKind, on_page: bool) -> PermissionResponse {
    if !on_page {
        return PermissionResponse::Deny;
    }
    match kind {
        PermissionKind::Camera | PermissionKind::Microphone | PermissionKind::Midi => {
            PermissionResponse::Allow
        }
        PermissionKind::DisplayCapture
        | PermissionKind::Geolocation
        | PermissionKind::Notifications => PermissionResponse::Deny,
        _ => PermissionResponse::Default,
    }
}

/// `name`, or `name (2).ext`, `name (3).ext`… the first that `taken` does not claim.
pub fn unique(dir: &Path, name: &str, taken: impl Fn(&Path) -> bool) -> PathBuf {
    let first = dir.join(name);
    if !taken(&first) {
        return first;
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(name);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !taken(p))
        .expect("an unbounded range")
}

/// The file name a download proposes: the host's suggestion, else the URL's last segment, never a path.
fn proposed_name(destination: &Path, url: &Url) -> String {
    let from_url = || {
        url.path_segments()
            .and_then(|mut s| s.next_back())
            .filter(|s| !s.is_empty() && !s.contains('\\'))
            .map(str::to_string)
    };
    destination
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .or_else(from_url)
        .unwrap_or_else(|| "download".into())
}

/// Where each download in flight is going: the macOS host never says it when it finishes.
static IN_FLIGHT: Mutex<Vec<(String, PathBuf)>> = Mutex::new(Vec::new());

fn port_of(app: &AppHandle) -> u16 {
    app.state::<SharedState>().lock().unwrap().port
}

pub fn install<'a>(builder: Builder<'a>, app: &AppHandle) -> Builder<'a> {
    let (nav, new, perm, dl) = (app.clone(), app.clone(), app.clone(), app.clone());
    builder
        .on_navigation(move |url| match route(url, port_of(&nav)) {
            Route::Stay => true,
            Route::Browser => {
                open::open_path(url.as_str());
                false
            }
            Route::Drop => false,
        })
        .on_new_window(move |url, _| {
            if route(&url, port_of(&new)) != Route::Drop {
                open::open_path(url.as_str());
            }
            NewWindowResponse::Deny
        })
        .on_permission_request(move |webview, kind| {
            let on_page = webview.url().is_ok_and(|u| is_page(&u, port_of(&perm)));
            permit(kind, on_page)
        })
        .on_download(move |_, event| on_download(&dl, event))
        .devtools(cfg!(debug_assertions))
}

fn on_download(app: &AppHandle, event: DownloadEvent<'_>) -> bool {
    match event {
        DownloadEvent::Requested { url, destination } => {
            let dir = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
            let name = proposed_name(destination, &url);
            let target = unique(&dir, &name, |p| p.exists());
            IN_FLIGHT
                .lock()
                .unwrap()
                .push((url.to_string(), target.clone()));
            *destination = target;
            true
        }
        DownloadEvent::Finished { url, path, success } => {
            let mut flight = IN_FLIGHT.lock().unwrap();
            let kept = flight.iter().position(|(u, _)| *u == url.as_str());
            let known = kept.map(|i| flight.remove(i).1);
            drop(flight);
            announce(app, path.or(known), success);
            true
        }
        _ => true,
    }
}

/// "Saved <name> in Downloads", with the way to the file; or that it failed.
fn announce(app: &AppHandle, saved: Option<PathBuf>, success: bool) {
    let name = saved
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let dialog = app.dialog().message(if success {
        format!("Saved {name} in Downloads")
    } else {
        "The download did not finish.".to_string()
    });
    let dialog = dialog.title("K7 DAW");
    match (success, saved) {
        (true, Some(path)) => dialog
            .buttons(MessageDialogButtons::OkCancelCustom(
                "Show in Folder".into(),
                "OK".into(),
            ))
            .show(move |reveal| {
                if reveal {
                    open::reveal(&path);
                }
            }),
        _ => dialog.show(|_| {}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(s: &str) -> Url {
        s.parse().unwrap()
    }

    #[test]
    fn k7_its_splash_and_blobs_stay_and_the_web_goes_to_the_browser() {
        assert_eq!(
            route(&u("http://127.0.0.1:18789/projects"), 18789),
            Route::Stay
        );
        assert_eq!(
            route(&u("tauri://localhost/index.html"), 18789),
            Route::Stay
        );
        assert_eq!(route(&u("http://tauri.localhost/"), 18789), Route::Stay);
        assert_eq!(
            route(&u("blob:http://127.0.0.1:18789/abc"), 18789),
            Route::Stay
        );
        assert_eq!(route(&u("https://k7.rocks/docs"), 18789), Route::Browser);
        assert_eq!(route(&u("http://127.0.0.1:9999/"), 18789), Route::Browser);
        assert_eq!(route(&u("mailto:a@b.c"), 18789), Route::Browser);
        assert_eq!(route(&u("file:///etc/passwd"), 18789), Route::Drop);
        assert_eq!(route(&u("javascript:alert(1)"), 18789), Route::Drop);
    }

    #[test]
    fn the_page_gets_media_and_midi_and_nothing_that_watches_the_screen() {
        assert_eq!(
            permit(PermissionKind::Camera, true),
            PermissionResponse::Allow
        );
        assert_eq!(
            permit(PermissionKind::Midi, true),
            PermissionResponse::Allow
        );
        assert_eq!(
            permit(PermissionKind::DisplayCapture, true),
            PermissionResponse::Deny
        );
        assert_eq!(
            permit(PermissionKind::Microphone, false),
            PermissionResponse::Deny
        );
    }

    #[test]
    fn a_download_never_overwrites_a_file() {
        let dir = Path::new("/d");
        let taken = |p: &Path| p == Path::new("/d/mix.wav") || p == Path::new("/d/mix (2).wav");
        assert_eq!(
            unique(dir, "mix.wav", taken),
            PathBuf::from("/d/mix (3).wav")
        );
        assert_eq!(unique(dir, "new.wav", taken), PathBuf::from("/d/new.wav"));
        assert_eq!(
            unique(dir, "README", |p| p == Path::new("/d/README")),
            PathBuf::from("/d/README (2)")
        );
    }

    #[test]
    fn a_proposed_name_is_a_name_not_a_path() {
        let url = u("http://127.0.0.1:1/api/files/song.zip");
        assert_eq!(
            proposed_name(Path::new("/tmp/x/take.wav"), &url),
            "take.wav"
        );
        assert_eq!(proposed_name(Path::new("/"), &url), "song.zip");
    }
}
