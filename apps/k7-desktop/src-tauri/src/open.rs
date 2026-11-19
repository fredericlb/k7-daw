// open.rs — hand an address, a folder or a file to the OS: the browser for k7, Finder or Explorer for a download, the
// editor for the log. The one thing a reader gets wrong: on Windows what must outlive the app starts outside its job.

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
pub fn open_text_file(path: &std::path::Path) {
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
