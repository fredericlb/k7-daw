// records.rs — a set list's lines against the workspace's audio files: the file each names, whether it is there with its four stems, the names a row shows.
// A reader would get wrong that a record's stems are `stems/` BESIDE it: one record per folder has its own, and a library ref (`@`) names no workspace record.

use crate::js_text::{is_js_space, js_trim};
use song_schema::{STEMS, STEMS_DIR, ext_of, is_audio, stem_file};
use std::collections::HashSet;

/// `path` split on `/`, or None when any segment is empty, `.` or `..`.
fn plain_segments(path: &str) -> Option<Vec<&str>> {
    let segs: Vec<&str> = path.split('/').collect();
    segs.iter()
        .all(|s| !s.is_empty() && *s != "." && *s != "..")
        .then_some(segs)
}

/// A line as the workspace path of its record, its slashes trimmed; None for one that leaves the workspace or names a library (`@`).
#[must_use]
pub fn record_path_of(line: &str) -> Option<String> {
    let clean = js_trim(line).trim_matches('/');
    if clean.is_empty() || clean.contains('\\') || clean.starts_with('@') {
        return None;
    }
    plain_segments(clean).map(|s| s.join("/"))
}

/// A ref the cue sheet may key a record by: a workspace path already in its plain form.
#[must_use]
pub fn is_record_ref(v: &str) -> bool {
    record_path_of(v).is_some_and(|p| p == v)
}

/// The folder a record's stems are in, workspace-relative with its trailing slash: `stems/` beside it.
#[must_use]
pub fn record_stems_dir(path: &str) -> String {
    let dir = path.rfind('/').map_or("", |i| &path[..=i]);
    format!("{dir}{STEMS_DIR}/")
}

/// What a row is called: the file's name without its extension.
#[must_use]
pub fn record_name(path: &str) -> &str {
    let base = path.rsplit('/').next().unwrap_or(path);
    match base.rfind('.') {
        Some(i) if i > 0 && !ext_of(base).is_empty() => &base[..i],
        _ => base,
    }
}

/// …and the folder holding it, or None for a record at the root.
#[must_use]
pub fn record_folder(path: &str) -> Option<&str> {
    let mut segs = path.rsplit('/');
    segs.next();
    segs.next()
}

/// Does this record match what was typed? Every word must be somewhere in its path.
#[must_use]
pub fn record_matches(path: &str, query: &str) -> bool {
    let hay = path.to_lowercase();
    js_trim(query)
        .to_lowercase()
        .split(is_js_space)
        .filter(|w| !w.is_empty())
        .all(|w| hay.contains(w))
}

/// One line, as far as the workspace's files take it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The record's workspace path, or None for a line that names none.
    pub path: Option<String>,
    /// Whether that audio file is in the workspace.
    pub present: bool,
    /// Whether its four stems are beside it.
    pub stems: bool,
}

/// One line against the workspace's files; None for `files` is a tree not read yet, which holds nothing.
#[must_use]
pub fn resolve_record(song: &str, files: Option<&HashSet<&str>>) -> Resolved {
    let path = record_path_of(song);
    let has = |p: &str| files.is_some_and(|f| f.contains(p));
    let present = path.as_deref().is_some_and(|p| is_audio(p) && has(p));
    let stems = present
        && path.as_deref().is_some_and(|p| {
            let dir = record_stems_dir(p);
            STEMS.iter().all(|s| has(&format!("{dir}{}", stem_file(*s))))
        });
    Resolved { path, present, stems }
}

/// The shape of a tree node this reads: a workspace path, a folder or not, and a folder's children.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeNode {
    pub path: String,
    pub dir: bool,
    pub children: Vec<TreeNode>,
}

/// A tree's audio files in tree order; a `stems/` folder's are left out unless asked, being no record.
#[must_use]
pub fn audio_files_of(tree: &[TreeNode], with_stems: bool) -> Vec<String> {
    let mut out = Vec::new();
    walk(tree, with_stems, &mut out);
    out
}

fn walk(nodes: &[TreeNode], with_stems: bool, out: &mut Vec<String>) {
    for n in nodes {
        if n.dir {
            let name = n.path.rsplit('/').next().unwrap_or(&n.path);
            if with_stems || name != STEMS_DIR {
                walk(&n.children, with_stems, out);
            }
        } else if is_audio(&n.path) {
            out.push(n.path.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files() -> HashSet<&'static str> {
        [
            "records/hells-bells/hells-bells.flac",
            "records/hells-bells/stems/drums.wav",
            "records/hells-bells/stems/bass.wav",
            "records/hells-bells/stems/other.wav",
            "records/hells-bells/stems/vocals.wav",
            "records/thunder.mp3",
            "records/stems/drums.wav",
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn a_workspace_path_keeps_its_form_with_its_slashes_trimmed() {
        assert_eq!(
            record_path_of("/records/thunder.mp3/").as_deref(),
            Some("records/thunder.mp3")
        );
        for bad in ["../x.wav", "a/./b.wav", "a\\b.wav", "  ", "@k7-core/songs/a", "a//b"] {
            assert_eq!(record_path_of(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_cue_sheets_ref_rule_is_the_plain_form() {
        assert!(is_record_ref("records/thunder.mp3"));
        assert!(!is_record_ref("/records/thunder.mp3"));
        assert!(!is_record_ref("@k7-core/songs/a/b/c"));
    }

    #[test]
    fn a_line_says_whether_it_is_there_and_has_its_four_stems() {
        let f = files();
        let at = |s: &str| {
            let r = resolve_record(s, Some(&f));
            (r.path, r.present, r.stems)
        };
        assert_eq!(
            at("records/hells-bells/hells-bells.flac"),
            (Some("records/hells-bells/hells-bells.flac".to_owned()), true, true)
        );
        assert_eq!(
            at("records/thunder.mp3"),
            (Some("records/thunder.mp3".to_owned()), true, false)
        );
        assert_eq!(
            at("records/gone.wav"),
            (Some("records/gone.wav".to_owned()), false, false)
        );
        assert_eq!(at("../escape.wav"), (None, false, false));
    }

    #[test]
    fn a_tree_not_read_holds_no_file_and_a_non_audio_file_is_no_record() {
        assert!(!resolve_record("records/thunder.mp3", None).present);
        let notes: HashSet<&str> = ["notes.txt"].into_iter().collect();
        assert!(!resolve_record("notes.txt", Some(&notes)).present);
    }

    #[test]
    fn a_row_shows_the_files_name_and_its_folder() {
        assert_eq!(record_name("records/hells-bells/Hells Bells.flac"), "Hells Bells");
        assert_eq!(record_name("noext"), "noext");
        assert_eq!(record_name(".hidden"), ".hidden");
        assert_eq!(record_folder("records/hells-bells/x.flac"), Some("hells-bells"));
        assert_eq!(record_folder("x.flac"), None);
        assert_eq!(record_stems_dir("records/x.flac"), "records/stems/");
        assert_eq!(record_stems_dir("x.flac"), "stems/");
    }

    #[test]
    fn every_word_typed_must_be_somewhere_in_the_path() {
        assert!(!record_matches("records/AC-DC/hells.flac", "acdc"));
        assert!(record_matches("records/AC-DC/hells.flac", "ac-dc hell"));
        assert!(record_matches("records/a.flac", "   "));
    }

    #[test]
    fn a_tree_flattens_to_its_audio_files_leaving_every_stems_folder_out() {
        let leaf = |p: &str| TreeNode {
            path: p.to_owned(),
            dir: false,
            children: vec![],
        };
        let tree = vec![TreeNode {
            path: "records".to_owned(),
            dir: true,
            children: vec![
                leaf("records/a.flac"),
                leaf("records/notes.txt"),
                TreeNode {
                    path: "records/stems".to_owned(),
                    dir: true,
                    children: vec![leaf("records/stems/drums.wav")],
                },
            ],
        }];
        assert_eq!(audio_files_of(&tree, false), ["records/a.flac"]);
        assert_eq!(
            audio_files_of(&tree, true),
            ["records/a.flac", "records/stems/drums.wav"]
        );
    }
}
