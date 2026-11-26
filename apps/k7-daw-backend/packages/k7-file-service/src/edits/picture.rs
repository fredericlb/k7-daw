//! picture.rs — the open project's PICTURE read by the `verbs` crate over the workspace: its document, and the scene the preview draws.
//! What a reader gets wrong: a picture that does not build answers `{error}` with a 200; only a mount that cannot be read fails the call.

use serde_json::Value;
use song::Cache;
pub use verbs::picture::Use;
use verbs::picture::{doc_answer, scene_answer};

use super::verb::{Line, mounts, project_dir};
use super::{StagedFs, env_for, env_of, posix};

/// A path made relative to the workspace, as the page's `/media` takes it; one outside stays as it is.
fn relative(root: &str, abs: &str) -> String {
    let root = posix(root);
    abs.strip_prefix(&format!("{}/", root.trim_end_matches('/')))
        .map_or_else(|| abs.to_owned(), str::to_owned)
}

/// `encodeURIComponent`: every byte but the unreserved ones written `%XX`.
#[must_use]
pub fn encode_component(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// The files the line's project reads from, staged for reading only.
fn reading(at: &Line<'_>) -> Result<(StagedFs, song_edit::confine::Env, String), String> {
    let env = at.project.map_or_else(
        || env_of(at.home, at.code_root),
        |p| env_for(at.home, at.code_root, p),
    );
    let (writes, reads) = mounts(at.home, at.root, &env);
    let fs = StagedFs::reading(&writes, &reads).map_err(|e| e.to_string())?;
    Ok((fs, env, project_dir(at)))
}

/// `{doc, warnings}` with every media path workspace-relative, or `{error}`.
///
/// # Errors
/// A mount that is not absolute.
pub fn picture_doc(at: &Line<'_>, cache: &mut Cache) -> Result<Value, String> {
    let (fs, env, dir) = reading(at)?;
    Ok(doc_answer(&fs, cache, &env, &dir, &|p| {
        relative(at.root, p)
    }))
}

/// `{scene, mixes}` the page draws or exports: each file a URL of its `/media`, `media` followed by the workspace path.
///
/// # Errors
/// A mount that is not absolute.
pub fn picture_scene(
    at: &Line<'_>,
    cache: &mut Cache,
    armed: Option<&str>,
    use_: Use,
    media: &str,
) -> Result<Value, String> {
    let (fs, env, dir) = reading(at)?;
    let url = |p: &str| {
        let rel = relative(at.root, p);
        format!("{media}{}", encode_component(&rel))
    };
    Ok(scene_answer(&fs, cache, &env, &dir, armed, use_, &url))
}

#[cfg(test)]
mod tests {
    use super::{encode_component, relative};

    #[test]
    fn a_media_path_is_relative_and_encoded_as_a_browser_does() {
        assert_eq!(
            relative("/w/ws", "/w/ws/song/clips/a b.mp4"),
            "song/clips/a b.mp4"
        );
        assert_eq!(relative("/w/ws", "/elsewhere/a.mp4"), "/elsewhere/a.mp4");
        assert_eq!(
            encode_component("song/clips/a b(1).mp4"),
            "song%2Fclips%2Fa%20b(1).mp4"
        );
    }
}
