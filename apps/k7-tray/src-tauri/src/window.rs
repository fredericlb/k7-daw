// window.rs — K7 DAW's one window: it opens on the bundled splash, navigates to the backend's page once the backend
// answers, and hides rather than closes (the engine keeps serving other devices; Quit is deliberate). The page gets
// no IPC and no capability: it is the same bundle every host serves. See docs/desktop.md.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};

use crate::{backend, handlers};

pub const LABEL: &str = "main";

/// The page is on its way or loaded: the splash no longer needs words.
static LOADED: AtomicBool = AtomicBool::new(false);
/// The backend went away while the page was up: the next ready reloads it.
static STALE: AtomicBool = AtomicBool::new(false);
/// A page asked for before the backend answered, opened instead of the root once it does.
static PENDING: Mutex<Option<String>> = Mutex::new(None);
/// The splash's last words, written again when the splash finishes loading.
static LAST: Mutex<String> = Mutex::new(String::new());

/// The marker the page reads (`window.k7Desktop`), the way the phones mark theirs.
fn marker() -> String {
    format!(
        "Object.defineProperty(window, 'k7Desktop', {{ value: Object.freeze({{ release: '{}' }}) }});",
        env!("K7_RELEASE")
    )
}

pub fn create(app: &AppHandle, visible: bool) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("K7 DAW")
        .inner_size(1440.0, 900.0)
        .min_inner_size(960.0, 600.0)
        .center()
        .visible(visible)
        .initialization_script(marker())
        // HTML5 drops (the file tree, the timeline) need the page to see them, not Tauri's native handler.
        .disable_drag_drop_handler()
        .on_page_load(move |win, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
                && !LOADED.load(Ordering::SeqCst)
            {
                let last = LAST.lock().unwrap().clone();
                say(&win, &last);
            }
        });
    // A scratch run keeps its web storage out of the person's: nothing persists.
    let builder = builder.incognito(crate::paths::scratch().is_some());
    let window = handlers::install(builder, app).build()?;
    #[cfg(target_os = "linux")]
    crate::webview_linux::configure(&window);
    let w = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = w.hide();
        }
    });
    Ok(window)
}

/// Bring the window up, on `path` of the page when one is given.
pub fn show(app: &AppHandle, path: Option<&str>) {
    if let Some(path) = path {
        if LOADED.load(Ordering::SeqCst) {
            if let (Some(win), Ok(url)) = (
                app.get_webview_window(LABEL),
                backend::url(app, path).parse(),
            ) {
                let _ = win.navigate(url);
            }
        } else {
            *PENDING.lock().unwrap() = Some(path.to_string());
        }
    }
    let win = app
        .get_webview_window(LABEL)
        .or_else(|| create(app, true).ok());
    if let Some(win) = win {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// What a backend standing asks of the window.
#[derive(Debug, PartialEq)]
enum Step {
    Say,
    MarkStale,
    Navigate,
    Reload,
    Nothing,
}

/// The splash speaks until the page is on its way; a gap in the backend makes the page stale; ready again reloads it.
fn step(ready: bool, loaded: bool, stale: bool) -> Step {
    match (ready, loaded, stale) {
        (false, false, _) => Step::Say,
        (false, true, _) => Step::MarkStale,
        (true, false, _) => Step::Navigate,
        (true, true, true) => Step::Reload,
        (true, true, false) => Step::Nothing,
    }
}

/// The backend's standing, from the tray's refresh.
pub fn on_status(app: &AppHandle, ready: bool, line: &str) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    let loaded = LOADED.load(Ordering::SeqCst);
    match step(ready, loaded, STALE.load(Ordering::SeqCst)) {
        Step::Say => say(&win, line),
        Step::MarkStale => STALE.store(true, Ordering::SeqCst),
        Step::Navigate => {
            LOADED.store(true, Ordering::SeqCst);
            let path = PENDING.lock().unwrap().take().unwrap_or_else(|| "/".into());
            if let Ok(url) = backend::url(app, &path).parse() {
                let _ = win.navigate(url);
            }
        }
        Step::Reload => {
            STALE.store(false, Ordering::SeqCst);
            let _ = win.reload();
        }
        Step::Nothing => {}
    }
}

/// Words on the splash; remembered, since the splash may not have finished loading.
fn say(win: &WebviewWindow, text: &str) {
    if text.is_empty() {
        return;
    }
    *LAST.lock().unwrap() = text.to_string();
    let json = serde_json::to_string(text).unwrap_or_default();
    let _ = win.eval(format!("window.__k7Status && window.__k7Status({json})"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_splash_speaks_until_the_page_is_on_its_way() {
        assert_eq!(step(false, false, false), Step::Say);
        assert_eq!(step(true, false, false), Step::Navigate);
    }

    #[test]
    fn a_gap_in_the_backend_reloads_the_page_once_it_is_back() {
        assert_eq!(step(false, true, false), Step::MarkStale);
        assert_eq!(step(true, true, true), Step::Reload);
        assert_eq!(step(true, true, false), Step::Nothing);
    }

    #[test]
    fn the_marker_names_the_release_and_cannot_be_overwritten() {
        let m = marker();
        assert!(m.contains(env!("K7_RELEASE")));
        assert!(m.contains("Object.freeze"));
    }
}
