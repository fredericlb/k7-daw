// appmenu.rs — the window's menu: the Mac's menu bar, and the menu bar of the window on Windows and Linux. It holds
// what only a menu reaches (Open in Browser, Engine Settings, Restart, Show Logs). The one thing a reader gets wrong:
// WKWebView gets Cmd-C/V/X/A/Z only through the Edit menu's key equivalents. See docs/desktop.md.

use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Manager, Wry};

use crate::{backend, open, paths, window};

const ENGINE: &str = "app_engine";
const BROWSER: &str = "app_browser";
const RESTART: &str = "app_restart";
const LOGS: &str = "app_logs";
const RELOAD: &str = "app_reload";
const DOCS: &str = "app_docs";
#[cfg(not(target_os = "macos"))]
const QUIT: &str = "app_quit";
#[cfg(target_os = "macos")]
const CLOSE: &str = "app_close";

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let item =
        |id: &str, text: &str, accel: Option<&str>| MenuItem::with_id(app, id, text, true, accel);
    let engine = item(ENGINE, "Engine Settings…", None)?;
    let browser = item(BROWSER, "Open in Browser", None)?;
    let restart = item(RESTART, "Restart K7 DAW", None)?;
    let logs = item(LOGS, "Show Logs", None)?;
    let docs = item(DOCS, "K7 DAW Help", None)?;
    let reload = item(RELOAD, "Reload", Some("CmdOrCtrl+R"))?;
    let sep = || PredefinedMenuItem::separator(app);
    let (s1, s4) = (sep()?, sep()?);

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
    let mut view_items: Vec<&dyn IsMenuItem<Wry>> = vec![&reload];
    #[cfg(target_os = "macos")]
    let fullscreen = PredefinedMenuItem::fullscreen(app, None)?;
    #[cfg(target_os = "macos")]
    view_items.push(&fullscreen);
    #[cfg(debug_assertions)]
    let devtools = item("app_devtools", "Developer Tools", Some("CmdOrCtrl+Alt+I"))?;
    #[cfg(debug_assertions)]
    view_items.push(&devtools);
    let view = Submenu::with_items(app, "View", true, &view_items)?;

    #[cfg(target_os = "macos")]
    let menu = {
        use tauri::menu::{AboutMetadata, HELP_SUBMENU_ID, WINDOW_SUBMENU_ID};
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
        let hide = PredefinedMenuItem::hide(app, None)?;
        let hide_others = PredefinedMenuItem::hide_others(app, None)?;
        let quit = PredefinedMenuItem::quit(app, Some("Quit K7 DAW"))?;
        let close = item(CLOSE, "Close Window", Some("CmdOrCtrl+W"))?;
        let (s2, s3) = (sep()?, sep()?);
        let app_menu = Submenu::with_items(
            app,
            "K7 DAW",
            true,
            &[
                &about,
                &s1,
                &engine,
                &browser,
                &restart,
                &s2,
                &hide,
                &hide_others,
                &s3,
                &quit,
            ],
        )?;
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
        Menu::with_items(app, &[&app_menu, &edit, &view, &win, &help])?
    };
    #[cfg(not(target_os = "macos"))]
    let menu = {
        let quit = item(QUIT, "Quit", Some("CmdOrCtrl+Q"))?;
        let file = Submenu::with_items(
            app,
            "File",
            true,
            &[&engine, &browser, &restart, &s1, &quit],
        )?;
        let help = Submenu::with_items(app, "Help", true, &[&docs, &logs])?;
        Menu::with_items(app, &[&file, &edit, &view, &help])?
    };
    app.set_menu(menu)?;
    app.on_menu_event(|app, event| on_menu(app, event.id.as_ref()));
    Ok(())
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        ENGINE => window::show(app, Some("/settings/engine")),
        BROWSER => open::open_path(&backend::url(app, "/")),
        RESTART => backend::restart(app),
        LOGS => open::open_text_file(&paths::merged_log_path()),
        DOCS => open::open_path(&backend::url(app, "/docs")),
        RELOAD => {
            if let Some(win) = app.get_webview_window(window::LABEL) {
                let _ = win.reload();
            }
        }
        #[cfg(not(target_os = "macos"))]
        QUIT => app.exit(0),
        #[cfg(target_os = "macos")]
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
