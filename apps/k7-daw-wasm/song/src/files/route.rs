// files/route.rs — which reader judges a workspace path, by FOLDER and BASENAME, the way the editor's router always read one.
// A reader would get wrong the order: `views/mix.k7x` shares its base with the root `mix.k7x`, so the folder test comes first.

/// The kind of file a path holds, and so the reader that judges it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    Preset,
    Ui,
    Controller,
    Config,
    Module,
    Chain,
    Graph,
    Clip,
    Sequence,
    Curve,
    Timeline,
    MixView,
    Project,
    Mix,
    Settings,
}

const SHARED_PRESETS_DIR: &str = "_presets";
const LIBRARY_PRESETS_DIR: &str = "presets";
const LIBRARIES_DIRS: [&str; 2] = ["_libraries", "libraries"];

/// `presets` as a library's family: two segments below `_libraries/` (virtual) or `libraries/` (on disk).
fn is_library_presets(dirs: &[&str], i: usize) -> bool {
    dirs[i] == LIBRARY_PRESETS_DIR && i >= 2 && LIBRARIES_DIRS.contains(&dirs[i - 2])
}

/// The base name without a `.k7x` extension, lowered.
fn base_of(name: &str) -> String {
    let n = name.len();
    let stem = match name.get(n.saturating_sub(4)..) {
        Some(ext) if n >= 4 && ext.eq_ignore_ascii_case(".k7x") => &name[..n - 4],
        _ => name,
    };
    stem.to_lowercase()
}

/// Whether a path names a `.k7x` file, whatever the case.
pub fn is_k7x(path: &str) -> bool {
    path.len() >= 4
        && path
            .get(path.len() - 4..)
            .is_some_and(|e| e.eq_ignore_ascii_case(".k7x"))
}

/// The family a path holds; None for a file no reader knows (a preset base, a dsp literal, a root file of no kind).
pub fn family_of(path: &str) -> Option<Family> {
    let normal = path.replace('\\', "/");
    let parts: Vec<&str> = normal.split('/').collect();
    let (name, dirs) = parts.split_last().map_or(("", &[][..]), |(n, d)| (*n, d));
    let base = base_of(name);
    let in_folder = |d: &str| dirs.contains(&d);

    if in_folder(SHARED_PRESETS_DIR) || (0..dirs.len()).any(|i| is_library_presets(dirs, i)) {
        return Some(Family::Preset);
    }
    if in_folder("modules") || in_folder("_modules") {
        return match base.as_str() {
            "ui" => Some(Family::Ui),
            // Read by nothing here: presets live in a presets tree, and a dsp literal is the compile's.
            "presets" | "dsp" => None,
            "controller" => Some(Family::Controller),
            "config" => Some(Family::Config),
            _ => Some(Family::Module),
        };
    }
    if in_folder("nodes") || in_folder("tracks") {
        return Some(Family::Module);
    }
    if in_folder("chains") {
        return Some(if base == "graph" { Family::Graph } else { Family::Chain });
    }
    for (dir, family) in [
        ("clips", Family::Clip),
        ("sequences", Family::Sequence),
        ("curves", Family::Curve),
        ("timelines", Family::Timeline),
    ] {
        if in_folder(dir) {
            return Some(family);
        }
    }
    if in_folder("views") && base == "mix" {
        return Some(Family::MixView);
    }
    match base.as_str() {
        "chains" => Some(Family::Graph),
        "project" => Some(Family::Project),
        "mix" => Some(Family::Mix),
        "settings" => Some(Family::Settings),
        _ => None,
    }
}
