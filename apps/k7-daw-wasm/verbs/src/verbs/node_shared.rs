// node_shared.rs — what the node verbs share: quoted words, the libraries' sound files, a node's chain and bypass.
// A reader would get wrong `library_files`: the project's own `samples/` answers as `@project/…`, ahead of every library.

use crate::host::Host;
use song::Cache;
use song::graph::load_graph;
use song::lanes::inputs::load_inputs;
use song::lanes::rig::Rig;
use song::modules::files::{PROJECT_SOUNDS, is_library_file_of};
use song_edit::Fs;
use song_edit::confine::Env;
use song_edit::list_files::{ListOpts, list_files_under};
use song_edit::loader::roots_for;
use song_edit::shared_folders::{Family, library_family_roots};

pub use super::words::split_args;

fn family_of(name: &str) -> Option<Family> {
    match name {
        "samples" => Some(Family::Samples),
        "irs" => Some(Family::Irs),
        "wavetables" => Some(Family::Wavetables),
        "models" => Some(Family::Models),
        _ => None,
    }
}

/// `libraryFiles`: every file of `family` in the project and each library, as `@<level>/<path>`, the project's first.
pub fn library_files(h: &Host, family: &str) -> Vec<String> {
    library_files_of(&*h.fs, &h.env, &h.song_path, family)
}

/// `library_files` over the files and roots alone, for a reader that holds no `Host` (the completion).
pub fn library_files_of(fs: &dyn Fs, env: &Env, song_path: &str, family: &str) -> Vec<String> {
    let Some(kind) = family_of(family) else {
        return Vec::new();
    };
    let mut roots = vec![(PROJECT_SOUNDS.to_owned(), format!("{song_path}/{family}"))];
    roots.extend(
        library_family_roots(fs, env, kind, Some(song_path))
            .into_iter()
            .map(|l| (l.name, l.root)),
    );
    let opts = ListOpts {
        max_depth: 6,
        ..ListOpts::default()
    };
    let mut own = Vec::new();
    let mut libs = Vec::new();
    for (name, root) in roots {
        for path in list_files_under(fs, &root, &opts) {
            let base = path.rsplit('/').next().unwrap_or(&path);
            if is_library_file_of(family, base) {
                let r = format!("@{name}/{path}");
                if name == PROJECT_SOUNDS {
                    own.push(r)
                } else {
                    libs.push(r)
                }
            }
        }
    }
    own.sort();
    libs.sort();
    own.extend(libs);
    own
}

/// The chain whose run holds `module`, and whether it plays (not bypassed); None when no chain names it.
pub fn chain_of_module(h: &Host, module: &str) -> Option<(String, bool)> {
    let roots = roots_for(&*h.fs, &h.env, &h.song_path);
    let mut cache = Cache::new();
    let inputs = load_inputs(&*h.fs, &mut cache, &roots, Rig::default()).ok()?;
    let g = load_graph(&*h.fs, &mut cache, &roots, &inputs).ok()?;
    g.chains.iter().find_map(|(name, c)| {
        c.use_steps
            .iter()
            .find(|s| s.module == module)
            .map(|s| (name.clone(), s.enabled != Some(false)))
    })
}

/// `pluginModule`: the module half of a compiled plugin id.
pub fn plugin_module(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, m)| m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_words_stay_whole() {
        assert_eq!(split_args(r#"kit "Big Room" BD"#), ["kit", "Big Room", "BD"]);
        assert_eq!(split_args("  a  b "), ["a", "b"]);
        assert_eq!(plugin_module("drums:comp"), "comp");
        assert_eq!(plugin_module("comp"), "comp");
    }
}
