// tray.rs — the icon and its menu: one status line (two while an instrument downloads), the window and the browser
// routes to k7, and the app's own switches. The VIEW: the words come from the backend's status route or from where
// the backend process stands, and every setting of the engine is a page of k7. See docs/tray.md.

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_autostart::ManagerExt;

use crate::state::{Backend, SharedState};
use crate::trayicon::{self, IconState};
use crate::{backend, paths, window};

pub struct MenuBuild {
    pub menu: Menu<Wry>,
    pub status: MenuItem<Wry>,
    pub downloads: MenuItem<Wry>,
    pub browser: MenuItem<Wry>,
    pub autostart: CheckMenuItem<Wry>,
}

pub fn build_menu(app: &AppHandle, autostart_on: bool) -> tauri::Result<MenuBuild> {
    let status = MenuItem::with_id(app, "status", "Starting K7 DAW…", false, None::<&str>)?;
    // Built once, inserted under the status line only while an instrument downloads (`refresh`).
    let downloads = MenuItem::with_id(app, "downloads", "", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open K7 DAW", true, None::<&str>)?;
    let browser = MenuItem::with_id(app, "browser", "Open in Browser", false, None::<&str>)?;
    let engine = MenuItem::with_id(app, "engine", "Engine Settings…", true, None::<&str>)?;
    let restart = MenuItem::with_id(app, "restart", "Restart K7 DAW", true, None::<&str>)?;
    let autostart = CheckMenuItem::with_id(
        app,
        "autostart",
        "Launch at Login",
        true,
        autostart_on,
        None::<&str>,
    )?;
    let logs = MenuItem::with_id(app, "logs", "Show Logs", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit K7 DAW", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);
    let (s1, s2, s3) = (sep()?, sep()?, sep()?);
    let items: Vec<&dyn IsMenuItem<Wry>> = vec![
        &status, &s1, &open, &browser, &engine, &restart, &s2, &autostart, &logs, &s3, &quit,
    ];
    let menu = Menu::with_items(app, &items)?;
    Ok(MenuBuild {
        menu,
        status,
        downloads,
        browser,
        autostart,
    })
}

/// The badge, the status line and the downloads line, from where the backend stands and what it last said.
fn shown(s: &crate::state::AppState) -> (IconState, String, Option<String>) {
    match &s.backend {
        Backend::Starting => (
            IconState::Loading(s.spinner),
            "Starting K7 DAW…".into(),
            None,
        ),
        Backend::Down(why) => (IconState::Error, format!("⚠ {why}"), None),
        Backend::Ready if s.said.line.is_empty() => (
            IconState::Loading(s.spinner),
            "Starting the engine…".into(),
            None,
        ),
        Backend::Ready => {
            let icon = match s.said.icon.as_str() {
                "running" => IconState::Running,
                "loading" => IconState::Loading(s.spinner),
                "error" => IconState::Error,
                _ => IconState::Stopped,
            };
            (icon, s.said.line.clone(), s.said.downloads.clone())
        }
    }
}

/// Advance the spinner and repaint the icon alone: nothing else changes between frames.
pub fn tick_spinner(app: &AppHandle) {
    let shared = app.state::<SharedState>();
    let (tray, icon) = {
        let mut s = shared.lock().unwrap();
        s.spinner = s.spinner.wrapping_add(1);
        let Some(m) = s.menus.as_ref() else { return };
        (m.tray.clone(), trayicon::render(shown(&s).0))
    };
    let _ = tray.set_icon(Some(icon));
}

/// Repaint everything the menu bar says. The lock is held only to read; Tauri is called with it released.
pub fn refresh(app: &AppHandle) {
    let shared = app.state::<SharedState>();
    let (status, line, downloads, toggle, browser, ready, tray, icon) = {
        let mut s = shared.lock().unwrap();
        let (state, line, downloads) = shown(&s);
        let icon = trayicon::render(state);
        let ready = s.backend == Backend::Ready;
        let Some(m) = s.menus.as_mut() else { return };
        let show = downloads.is_some();
        let toggle = (show != m.downloads_shown).then(|| (m.menu.clone(), show));
        m.downloads_shown = show;
        (
            m.status.clone(),
            line,
            (m.downloads.clone(), downloads),
            toggle,
            m.browser.clone(),
            ready,
            m.tray.clone(),
            icon,
        )
    };
    let _ = status.set_text(&line);
    let (item, text) = downloads;
    if let Some(text) = text {
        let _ = item.set_text(text);
    }
    if let Some((menu, show)) = toggle {
        let _ = if show {
            menu.insert(&item, 1)
        } else {
            menu.remove(&item)
        };
    }
    let _ = browser.set_enabled(ready);
    let _ = tray.set_icon(Some(icon));
    let _ = tray.set_tooltip(Some(format!("K7 DAW — {line}")));
    window::on_status(app, ready, &line);
}

pub fn on_menu(app: &AppHandle, id: &str) {
    match id {
        "open" => window::show(app, None),
        "browser" => open_path(&backend::url(app, "/")),
        "engine" => window::show(app, Some("/settings/engine")),
        "restart" => backend::restart(app),
        "autostart" => toggle_autostart(app),
        "logs" => open_text_file(&paths::merged_log_path()),
        "quit" => app.exit(0),
        _ => {}
    }
}

fn toggle_autostart(app: &AppHandle) {
    let mgr = app.autolaunch();
    let _ = if mgr.is_enabled().unwrap_or(false) {
        mgr.disable()
    } else {
        mgr.enable()
    };
    if let Some(m) = app.state::<SharedState>().lock().unwrap().menus.as_ref() {
        let _ = m.autostart.set_checked(mgr.is_enabled().unwrap_or(false));
    }
}

/// Whether `target` is an address (`scheme://…` or `mailto:`), and so never one of a program's own switches.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn is_address(target: &str) -> bool {
    (target.contains("://") || target.starts_with("mailto:"))
        && !target.starts_with(['-', '/'])
        && !target.contains(['\n', '\r', '\0'])
}

/// A folder, a file or an address, in whatever the OS opens it with — the browser, for k7.
pub fn open_path(target: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(target).spawn();
    // Explorer opens a URL through the shell's association, with no command line to interpret `&` or `|`; out of
    // the tray's job, since a browser this starts is the person's.
    #[cfg(target_os = "windows")]
    if is_address(target) {
        let _ = crate::job::spawn_outside(std::process::Command::new("explorer").arg(target));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std::process::Command::new("xdg-open").arg(target).spawn();
}

/// A file, shown in its folder: selected in Finder or Explorer, its folder opened where there is no such thing.
pub fn reveal(path: &std::path::Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .spawn();
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // Explorer wants the quotes inside its own argument, which `arg` would escape.
        let _ = crate::job::spawn_outside(
            std::process::Command::new("explorer")
                .raw_arg(format!("/select,\"{}\"", path.display())),
        );
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    if let Some(dir) = path.parent() {
        open_path(&dir.to_string_lossy());
    }
}

/// A text file: Notepad on Windows, where a `.log` usually has no association.
fn open_text_file(path: &std::path::Path) {
    #[cfg(target_os = "windows")]
    let _ = crate::job::spawn_outside(std::process::Command::new("notepad.exe").arg(path));
    #[cfg(not(target_os = "windows"))]
    open_path(&path.to_string_lossy());
}

#[cfg(test)]
mod tests {
    use super::is_address;

    #[test]
    fn only_addresses_are_handed_to_the_shell_and_metacharacters_stay_inert() {
        assert!(is_address("https://example.com/?a=1&calc|x^y"));
        assert!(is_address("mailto:a@b.c?subject=a&body=b"));
        assert!(!is_address("/select,C:\\Windows"));
        assert!(!is_address("calc.exe"));
        assert!(!is_address("-k"));
        assert!(!is_address("https://a.b/\nx"));
    }
}
