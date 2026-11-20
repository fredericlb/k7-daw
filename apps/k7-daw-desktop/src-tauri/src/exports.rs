// exports.rs — the front's folder picker and file manager, answered with Tauri's dialog and the OS's own.
// The one thing a reader would get wrong: the dialog blocks, so the front calls it off its async threads.

use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::open;

struct Dialogs(AppHandle);

impl k7_front::export::Host for Dialogs {
    fn pick_folder(&self, start: Option<&Path>) -> Option<PathBuf> {
        let mut dialog = self.0.dialog().file().set_title("Export to");
        if let Some(dir) = start {
            dialog = dialog.set_directory(dir);
        }
        dialog.blocking_pick_folder()?.into_path().ok()
    }

    fn reveal(&self, path: &Path) {
        if path.is_dir() {
            open::open_path(&path.to_string_lossy());
        } else {
            open::reveal(path);
        }
    }
}

/// Before the front starts: the page's "Choose a folder…" opens this app's dialog.
pub fn install(app: &AppHandle) {
    k7_front::export::set_host(std::sync::Arc::new(Dialogs(app.clone())));
}
