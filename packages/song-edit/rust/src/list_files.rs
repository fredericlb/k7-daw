// list_files.rs — every file under a folder as root-relative `/` paths, depth first; an unreadable folder lists nothing.
// A reader would get wrong the order: it is `Fs::list`'s (bytewise), where a TypeScript `readdir` gives disk order.

use crate::fs::Fs;
use crate::paths::join;
use song::Kind;

/// What `listFilesUnder` accepts; a link is already followed by `Fs::list`, so a file is always regular.
#[derive(Clone, Copy, Debug)]
pub struct ListOpts<'a> {
    pub max_depth: usize,
    pub skip_dirs: &'a [&'a str],
    pub dotfiles: bool,
}

impl Default for ListOpts<'_> {
    fn default() -> Self {
        ListOpts {
            max_depth: usize::MAX,
            skip_dirs: &[],
            dotfiles: false,
        }
    }
}

fn walk(fs: &dyn Fs, abs: &str, rel: &str, depth: usize, opts: &ListOpts, out: &mut Vec<String>) {
    let Ok(entries) = fs.list(abs) else { return };
    for e in entries {
        if !opts.dotfiles && e.name.starts_with('.') {
            continue;
        }
        let child = if rel.is_empty() {
            e.name.clone()
        } else {
            format!("{rel}/{}", e.name)
        };
        if e.kind == Kind::Dir {
            if depth < opts.max_depth && !opts.skip_dirs.contains(&e.name.as_str()) {
                walk(fs, &join(&[abs, &e.name]), &child, depth + 1, opts, out);
            }
        } else {
            out.push(child);
        }
    }
}

/// The files under `root`, relative to it.
#[must_use]
pub fn list_files_under(fs: &dyn Fs, root: &str, opts: &ListOpts) -> Vec<String> {
    let mut out = Vec::new();
    walk(fs, root, "", 0, opts, &mut out);
    out
}
