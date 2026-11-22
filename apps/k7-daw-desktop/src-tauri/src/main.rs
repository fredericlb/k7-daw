// main.rs — K7 DAW, the desktop app: a Tauri app with one window on k7. It starts k7's backend on 127.0.0.1, which
// runs the machine's supervisor and engine. The Windows job object is created HERE, before anything spawns, so a
// force-killed app takes the whole stack with it. See docs/app.md and docs/desktop.md.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod appmenu;
mod backend;
mod exports;
mod handlers;
mod http;
#[cfg(target_os = "macos")]
mod icloud;
mod idmove;
mod job;
mod logbus;
mod open;
mod paths;
mod pidkill;
mod state;
#[cfg(target_os = "linux")]
mod webview_linux;
mod window;

use std::sync::{Arc, Mutex};
use tauri::{Manager, RunEvent};

use state::{AppState, Backend, SharedState};

fn main() {
    #[cfg(target_os = "linux")]
    webview_linux::before_gtk();
    // Before the builder and its plugins create the new folders; a scratch run leaves the person's alone.
    let moved = if paths::scratch().is_none() {
        idmove::run()
    } else {
        Vec::new()
    };
    let mut builder = tauri::Builder::default();
    // A second launch brings the window up; a scratch run (`K7_DESKTOP_SCRATCH`) is a second app on purpose.
    if paths::scratch().is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show(app, None);
        }));
    }
    builder = builder.plugin(tauri_plugin_dialog::init());
    // A scratch run keeps its window state out of the person's config folder.
    if paths::scratch().is_none() {
        builder = builder.plugin(window_state());
    }
    builder
        .manage(backend::Process::default())
        .setup(move |app| {
            let handle = app.handle().clone();
            exports::install(&handle);

            // Before anything spawns: job membership is inherited, so the whole stack dies with the app.
            let job = job::init();
            // One launch is one session of the merged log, truncated before the backend appends to it.
            logbus::init();
            if let Err(why) = job {
                logbus::line("host", &why);
            }
            logbus::line("host", &format!("K7 DAW {} starting", env!("K7_RELEASE")));
            for l in &moved {
                logbus::line("host", l);
            }
            // The front runs in this process, whose stderr nobody reads: its panics and its stop go to the merged log.
            k7_front::exit::watch(Some(|l| logbus::line("front", l)));

            let shared: SharedState = Arc::new(Mutex::new(AppState {
                backend: Backend::Starting,
                port: backend::port(&handle),
            }));
            app.manage(shared);

            // Before the front starts: its keychain and its roaming store are chosen once.
            #[cfg(target_os = "macos")]
            logbus::line("host", icloud::install());

            appmenu::install(&handle)?;
            window::create(&handle)?;
            backend::spawn(&handle);

            // SIGTERM/SIGINT stop like Quit: a logout and Ctrl-C in dev end the app that way.
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

/// Size, position and maximized state are kept; visibility is not (a window closed on a Mac is not hidden next launch).
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
