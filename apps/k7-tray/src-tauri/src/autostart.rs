// autostart.rs — launch at login: the login item carries `--autostart`, so a login starts K7 DAW hidden in the
// tray; an entry written before the window existed (no argument) is registered again once, and the entry the
// tray was registered under (`k7-tray`) is retired in favour of K7 DAW's own.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::paths;

/// The argument the login item passes.
pub const ARG: &str = "--autostart";

/// Whether this launch is the login item's: the window stays hidden.
pub fn launched_at_login() -> bool {
    std::env::args().any(|a| a == ARG)
}

/// An enabled entry without the argument is registered again, once; the marker says it was.
pub fn reregister(app: &AppHandle) {
    // A scratch run must not touch the person's login item.
    if paths::scratch().is_some() {
        return;
    }
    let marker = paths::backend_root(app).join("autostart-args");
    let mgr = app.autolaunch();
    if marker.exists() || !mgr.is_enabled().unwrap_or(false) {
        return;
    }
    if mgr.disable().is_ok() && mgr.enable().is_ok() {
        let _ = std::fs::create_dir_all(paths::backend_root(app));
        let _ = std::fs::write(marker, ARG);
    }
}

/// The plugin: the entry's name and argument pinned, a LaunchAgent on macOS.
pub fn plugin<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    let builder = tauri_plugin_autostart::Builder::new()
        .app_name(NAME)
        .arg(ARG);
    #[cfg(target_os = "macos")]
    let builder = builder.macos_launcher(tauri_plugin_autostart::MacosLauncher::LaunchAgent);
    builder.build()
}

/// What the login item is called, pinned: it is the entry's key on every OS.
pub const NAME: &str = "K7 DAW";
/// The name the windowless tray registered under, before the product was renamed.
const OLD_NAME: &str = "k7-tray";

/// An install that was launching at login as the tray now launches as K7 DAW, and the old entry goes.
pub fn retire_old(app: &AppHandle) {
    if paths::scratch().is_some() {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let mut old = auto_launch::AutoLaunchBuilder::new();
    old.set_app_name(OLD_NAME)
        .set_app_path(&exe.display().to_string());
    #[cfg(target_os = "macos")]
    old.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    let Ok(old) = old.build() else { return };
    if old.is_enabled().unwrap_or(false) && old.disable().is_ok() {
        let _ = app.autolaunch().enable();
    }
}
