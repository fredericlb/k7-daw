// autostart.rs — launch at login: the login item carries `--autostart`, so a login starts K7 DAW hidden in the
// tray; an entry written before the window existed (no argument) is written again once, and the entry the tray
// was registered under (`k7-tray`) moves to K7 DAW's own, the old one removed last.

use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

use crate::paths;

/// The argument the login item passes.
pub const ARG: &str = "--autostart";

/// Whether this launch is the login item's: the window stays hidden.
pub fn launched_at_login() -> bool {
    std::env::args().any(|a| a == ARG)
}

/// One login entry, as the migration sees it.
trait Login {
    fn on(&self) -> bool;
    fn enable(&self) -> bool;
    fn disable(&self) -> bool;
}

impl Login for tauri_plugin_autostart::AutoLaunchManager {
    fn on(&self) -> bool {
        self.is_enabled().unwrap_or(false)
    }
    fn enable(&self) -> bool {
        tauri_plugin_autostart::AutoLaunchManager::enable(self).is_ok()
    }
    fn disable(&self) -> bool {
        tauri_plugin_autostart::AutoLaunchManager::disable(self).is_ok()
    }
}

impl Login for auto_launch::AutoLaunch {
    fn on(&self) -> bool {
        self.is_enabled().unwrap_or(false)
    }
    fn enable(&self) -> bool {
        auto_launch::AutoLaunch::enable(self).is_ok()
    }
    fn disable(&self) -> bool {
        auto_launch::AutoLaunch::disable(self).is_ok()
    }
}

/// An enabled entry without the argument is written again, once: enable overwrites, so a failure loses nothing.
fn rewrite(entry: &dyn Login, marker: &std::path::Path) {
    if marker.exists() || !entry.on() {
        return;
    }
    if entry.enable() {
        let _ = std::fs::create_dir_all(marker.parent().unwrap_or(std::path::Path::new(".")));
        let _ = std::fs::write(marker, ARG);
    }
}

/// The old entry's person gets K7 DAW's; the old one goes only once the new one is in place.
fn migrate(old: &dyn Login, new: &dyn Login) {
    // A failed enable or disable leaves the old entry as it was: the next start tries again.
    if old.on() && (new.on() || new.enable()) {
        old.disable();
    }
}

/// Before the menu reads the entry: both migrations of the login item.
pub fn settle(app: &AppHandle) {
    // A scratch run must not touch the person's login item.
    if paths::scratch().is_some() {
        return;
    }
    let new = app.autolaunch();
    if let Some(old) = old_entry() {
        migrate(&old, &*new);
    }
    rewrite(&*new, &paths::backend_root(app).join("autostart-args"));
}

fn old_entry() -> Option<auto_launch::AutoLaunch> {
    let exe = std::env::current_exe().ok()?;
    let mut old = auto_launch::AutoLaunchBuilder::new();
    old.set_app_name(OLD_NAME)
        .set_app_path(&exe.display().to_string());
    #[cfg(target_os = "macos")]
    old.set_macos_launch_mode(auto_launch::MacOSLaunchMode::LaunchAgent);
    old.build().ok()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct Fake {
        on: Cell<bool>,
        enable_ok: bool,
        disable_ok: bool,
        enabled_calls: Cell<u32>,
        disabled_calls: Cell<u32>,
    }
    impl Fake {
        fn new(on: bool, enable_ok: bool) -> Self {
            Fake {
                on: Cell::new(on),
                enable_ok,
                disable_ok: true,
                enabled_calls: Cell::new(0),
                disabled_calls: Cell::new(0),
            }
        }
    }
    impl Login for Fake {
        fn on(&self) -> bool {
            self.on.get()
        }
        fn enable(&self) -> bool {
            self.enabled_calls.set(self.enabled_calls.get() + 1);
            if self.enable_ok {
                self.on.set(true);
            }
            self.enable_ok
        }
        fn disable(&self) -> bool {
            self.disabled_calls.set(self.disabled_calls.get() + 1);
            if self.disable_ok {
                self.on.set(false);
            }
            self.disable_ok
        }
    }

    #[test]
    fn the_old_entry_goes_only_once_the_new_one_is_in() {
        let (old, new) = (Fake::new(true, true), Fake::new(false, false));
        migrate(&old, &new);
        assert!(old.on.get(), "a failed enable must leave the old entry");
        let (old, new) = (Fake::new(true, true), Fake::new(false, true));
        migrate(&old, &new);
        assert!(new.on.get() && !old.on.get());
    }

    #[test]
    fn a_login_that_was_off_stays_off() {
        let (old, new) = (Fake::new(false, true), Fake::new(false, true));
        migrate(&old, &new);
        assert!(!new.on.get());
    }

    #[test]
    fn rewriting_never_disables_and_marks_only_after_success() {
        let dir = std::env::temp_dir().join(format!("k7-autostart-{}", std::process::id()));
        let marker = dir.join("marker");
        let _ = std::fs::remove_dir_all(&dir);
        let failing = Fake::new(true, false);
        rewrite(&failing, &marker);
        assert_eq!(failing.disabled_calls.get(), 0);
        assert!(failing.on.get() && !marker.exists());
        let ok = Fake::new(true, true);
        rewrite(&ok, &marker);
        assert_eq!(ok.disabled_calls.get(), 0);
        assert!(marker.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
