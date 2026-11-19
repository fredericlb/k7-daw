// appmenu.rs — the macOS menu bar. WKWebView gets its Cmd-C/V/X/A/Z only through the Edit menu's key equivalents,
// and a Mac app without an app menu and Cmd-Q is not one. Windows and Linux have no menu bar. See docs/desktop.md.

use tauri::menu::{
    AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Wry};

use crate::{backend, tray, window};

/// The ids this menu answers to; the tray's own ids are the tray's.
const ENGINE: &str = "app_engine";
const LOGS: &str = "app_logs";
const RELOAD: &str = "app_reload";
const CLOSE: &str = "app_close";
const DOCS: &str = "app_docs";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let item =
        |id: &str, text: &str, accel: Option<&str>| MenuItem::with_id(app, id, text, true, accel);
    let engine = item(ENGINE, "Engine Settings…", None)?;
    let logs = item(LOGS, "Show Logs", None)?;
    let reload = item(RELOAD, "Reload", Some("CmdOrCtrl+R"))?;
    let close = item(CLOSE, "Close Window", Some("CmdOrCtrl+W"))?;
    let docs = item(DOCS, "K7 DAW Help", None)?;
    let sep = || PredefinedMenuItem::separator(app);
    let about = PredefinedMenuItem::about(
        app,
        Some("About K7 DAW"),
        Some(AboutMetadata {
            name: Some("K7 DAW".into()),
            version: Some(env!("K7_RELEASE").into()),
            website: Some("https://k7.rocks".into()),
            ..Default::default()
        }),
    )?;
    let (s1, s2, s3, s4) = (sep()?, sep()?, sep()?, sep()?);
    let mut first: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = vec![&about, &s1, &engine];
    let hide = PredefinedMenuItem::hide(app, None)?;
    let hide_others = PredefinedMenuItem::hide_others(app, None)?;
    let quit = PredefinedMenuItem::quit(app, Some("Quit K7 DAW"))?;
    first.extend([
        &s2 as &dyn tauri::menu::IsMenuItem<Wry>,
        &hide,
        &hide_others,
        &s3,
        &quit,
    ]);
    let app_menu = Submenu::with_items(app, "K7 DAW", true, &first)?;

    let edit = Submenu::with_items(
        app,
        "Edit",
        true,
        &[
            &PredefinedMenuItem::undo(app, None)?,
            &PredefinedMenuItem::redo(app, None)?,
            &s4,
            &PredefinedMenuItem::cut(app, None)?,
            &PredefinedMenuItem::copy(app, None)?,
            &PredefinedMenuItem::paste(app, None)?,
            &PredefinedMenuItem::select_all(app, None)?,
        ],
    )?;
    let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;
    let mut view_items: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = vec![&reload, &fullscreen];
    #[cfg(debug_assertions)]
    let devtools = item("app_devtools", "Developer Tools", Some("CmdOrCtrl+Alt+I"))?;
    #[cfg(debug_assertions)]
    view_items.push(&devtools);
    let view = Submenu::with_items(app, "View", true, &view_items)?;
    let win = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        "Window",
        true,
        &[
            &PredefinedMenuItem::minimize(app, None)?,
            &PredefinedMenuItem::maximize(app, Some("Zoom"))?,
            &close,
        ],
    )?;
    let help = Submenu::with_id_and_items(app, HELP_SUBMENU_ID, "Help", true, &[&docs, &logs])?;
    let menu = Menu::with_items(app, &[&app_menu, &edit, &view, &win, &help])?;
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| on_menu(app, event.id.as_ref()));
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        ENGINE => window::show(app, Some("/settings/engine")),
        LOGS => tray::on_menu(app, "logs"),
        DOCS => tray::open_path(&backend::url(app, "/docs")),
        RELOAD => {
            if let Some(win) = app.get_webview_window(window::LABEL) {
                let _ = win.reload();
            }
        }
        CLOSE => {
            if let Some(win) = app.get_webview_window(window::LABEL) {
                let _ = win.hide();
            }
        }
        #[cfg(debug_assertions)]
        "app_devtools" => {
            if let Some(win) = app.get_webview_window(window::LABEL) {
                win.open_devtools();
            }
        }
        _ => {}
    }
}
