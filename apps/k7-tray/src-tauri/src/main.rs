// main.rs — K7 DAW, the desktop app: a Tauri app with one window and a tray icon. It starts k7's backend on
// 127.0.0.1, which runs the machine's supervisor and engine, and shows k7's page in its window. The Windows job
// object is created HERE, before anything spawns, so a force-killed app takes the whole stack with it.
// See docs/tray.md and docs/desktop.md.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

#[cfg(target_os = "macos")]
mod appmenu;
mod autostart;
mod backend;
mod handlers;
mod http;
mod job;
mod logbus;
mod loops;
mod paths;
mod pidkill;
mod state;
mod tray;
mod trayicon;
mod updates;
#[cfg(target_os = "linux")]
mod webview_linux;
mod window;

use std::sync::{Arc, Mutex};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent};
use tauri_plugin_autostart::ManagerExt;

use state::{AppState, Backend, Menus, SharedState};

fn main() {
    #[cfg(target_os = "linux")]
    webview_linux::before_gtk();
    let mut builder = tauri::Builder::default();
    // A second launch brings the window up; a scratch run (`K7_TRAY_SCRATCH`) is a second app on purpose.
    if paths::scratch().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show(app, None);
        }));
    }
    builder = builder
        .plugin(autostart::plugin())
        .plugin(tauri_plugin_dialog::init());
    // A scratch run keeps its window state out of the person's config folder.
    if paths::scratch().is_none() {
        builder = builder.plugin(window_state());
    }
    if let Some(updater) = updates::plugin() {
        builder = builder.plugin(updater);
    }
    builder
        .manage(backend::Process::default())
        .setup(|app| {
            let handle = app.handle().clone();

            // Before anything spawns: job membership is inherited, so the whole stack dies with the tray.
            let job = job::init();
            // One tray launch is one session of the merged log, truncated before the backend appends to it.
            logbus::init();
            if let Err(why) = job {
                logbus::line("host", &why);
            }
            logbus::line("host", &format!("K7 DAW {} starting", env!("K7_RELEASE")));
            // The front runs in this process, whose stderr nobody reads: its panics and its stop go to the merged log.
            k7_front::exit::watch(Some(|l| logbus::line("front", l)));

            // The login entry is settled first: the menu's check reads what is on disk after the migration.
            autostart::settle(&handle);
            let autostart_on = handle.autolaunch().is_enabled().unwrap_or(false);
            let mb = tray::build_menu(&handle, autostart_on, updates::auto_enabled(&handle))?;
            let icon = TrayIconBuilder::with_id("main")
                .icon(trayicon::render(trayicon::IconState::Loading(0)))
                .tooltip("K7 DAW")
                .menu(&mb.menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| tray::on_menu(app, event.id.as_ref()))
                .build(&handle)?;

            let shared: SharedState = Arc::new(Mutex::new(AppState {
                backend: Backend::Starting,
                said: Default::default(),
                token: backend::random_hex(32),
                port: backend::port(&handle),
                spinner: 0,
                menus: Some(Menus {
                    status: mb.status,
                    downloads: mb.downloads,
                    downloads_shown: false,
                    browser: mb.browser,
                    menu: mb.menu,
                    autostart: mb.autostart,
                    auto_update: mb.auto_update,
                    tray: icon,
                }),
            }));
            app.manage(shared);

            #[cfg(target_os = "macos")]
            appmenu::install(&handle)?;
            // A login starts hidden; any other launch shows the splash at once.
            window::create(&handle, !autostart::launched_at_login())?;
            backend::spawn(&handle);
            loops::start(&handle);
            updates::start(&handle);

            // SIGTERM/SIGINT stop like Quit: launchd stops a LaunchAgent that way, and Ctrl-C in dev should too.
            #[cfg(unix)]
            {
                let h = handle.clone();
                tauri::async_runtime::spawn(async move {
                    use tokio::signal::unix::{signal, SignalKind};
                    let mut term = signal(SignalKind::terminate()).expect("SIGTERM");
                    let mut int = signal(SignalKind::interrupt()).expect("SIGINT");
                    tokio::select! {
                        _ = term.recv() => {},
                        _ = int.recv() => {},
                    }
                    cleanup(&h);
                    std::process::exit(0);
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building K7 DAW")
        .run(|app, event| {
            match event {
                // Cmd-Q and a logout end in Exit alone, which no ExitRequested precedes.
                RunEvent::ExitRequested { .. } | RunEvent::Exit => cleanup(app),
                // The Dock icon of a hidden window brings it back.
                #[cfg(target_os = "macos")]
                RunEvent::Reopen { .. } => window::show(app, None),
                _ => {}
            }
        });
}

/// Size, position and maximized state are kept; visibility is the launch's (a login starts hidden).
fn window_state<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    use tauri_plugin_window_state::StateFlags;
    tauri_plugin_window_state::Builder::default()
        .with_state_flags(StateFlags::all() & !StateFlags::VISIBLE)
        .build()
}

/// Whether this is the first call: the stop must run once, whichever event reaches it first.
fn first(flag: &std::sync::atomic::AtomicBool) -> bool {
    !flag.swap(true, std::sync::atomic::Ordering::SeqCst)
}

static CLEANED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// On quit: the backend stops its supervisor, which deregisters and stops the engine; the backstop catches what a
/// hung one left behind.
fn cleanup(app: &tauri::AppHandle) {
    if !first(&CLEANED) {
        return;
    }
    logbus::line("host", "quitting");
    backend::stop(app);
    pidkill::backstop();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stop_runs_once_whatever_event_comes_first() {
        let flag = std::sync::atomic::AtomicBool::new(false);
        assert!(first(&flag));
        assert!(!first(&flag));
    }
}
