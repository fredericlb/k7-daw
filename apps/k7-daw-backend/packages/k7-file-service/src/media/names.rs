//! names.rs — where a model lands, whether a take is inside its record directory, the name a take lands under, and the
//! prefetch's signature (2444, 2451, 2460). Strings only, through node's path model. What a reader gets wrong:
//! `C:/x` is absolute here on every host, as `path.ts`' `isAbsolute` reads it.

use crate::paths::{is_absolute, is_below, is_within, join, resolve, resolve_all};

/// `modelsRoot`: `$STUDIO_MODELS` (empty counts as unset), else the studio home's `models`, resolved.
pub fn models_root(models: Option<&str>, studio_home: &str) -> String {
    match models.filter(|m| !m.is_empty()) {
        Some(m) => resolve_all(&[m]),
        None => resolve_all(&[&join(&[studio_home, "models"])]),
    }
}

/// `modelCachePath`: where `file` lands under `root`, or none for a name that would leave it, which is refused rather than cleaned.
pub fn model_cache_path(root: &str, file: &str) -> Option<String> {
    if file.is_empty() || is_absolute(file) {
        return None;
    }
    let (root, at) = (resolve_all(&[root]), resolve(root, file));
    is_below(&root, &at).then_some(at)
}

/// `insideDir`: `file` is inside `dir`, both resolved so `..` cannot walk out of it.
pub fn inside_dir(dir: &str, file: &str) -> bool {
    !dir.is_empty() && is_within(&resolve_all(&[dir]), &resolve_all(&[file]))
}

/// What a take's name decides before the disk is asked (2451).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Take {
    /// The engine wrote it in the record directory: this is the file.
    Here(String),
    /// A path from a mirror: only its name is kept, and this file of that name is what must land.
    Land(String),
    /// Nothing can be opened for it.
    Refused,
}

/// `takeHere`'s first half: the take as this project holds it, before any wait for the tunnel.
pub fn take_name(record_dir: &str, file: &str) -> Take {
    if inside_dir(record_dir, file) {
        return Take::Here(file.to_string());
    }
    let name = file.rsplit(['/', '\\']).next().unwrap_or("");
    if record_dir.is_empty() || name.is_empty() || name == "." || name == ".." {
        return Take::Refused;
    }
    Take::Land(join(&[record_dir, name]))
}

/// The sentence a refused take answers.
pub fn outside_sentence(file: &str) -> String {
    format!(
        "\"{file}\" is not one of this project's takes — a recording is saved from its own record directory"
    )
}

/// `prefetchOnce`'s signature: the jobs address and the sorted files, which a list that did not move repeats.
pub fn prefetch_signature(jobs_url: &str, files: &[String]) -> String {
    format!("{jobs_url}\n{}", files.join("\n"))
}

#[cfg(test)]
mod root_tests {
    use super::model_cache_path;

    // A backslash root (the host's spelling) holds the model the slashed path names.
    #[test]
    fn a_backslash_root_holds_its_models() {
        assert_eq!(
            model_cache_path("C:\\m", "a/m.onnx").as_deref(),
            Some("C:/m/a/m.onnx")
        );
        assert_eq!(model_cache_path("C:\\m", "../x"), None);
    }
}
