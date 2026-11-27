// reload.rs — what a command's file list says about which routes still have to be re-asked.
// A reader would get wrong that a rewrite is always skippable: only a rewrite of a by-name entity leaves the vocabulary alone.

use serde::{Deserialize, Serialize};
use song_edit::sugar::is_doc_file;

/// How a command changed one file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Change {
    Added,
    Edited,
    Removed,
}

/// One file a command wrote.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ChangedFile {
    pub path: String,
    pub kind: Change,
}

/// The question a route asks of a reload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Did a file appear or disappear? The only thing `/tree` draws.
    Tree,
    /// Could this have changed `/hints`?
    Vocabulary,
    /// Could this have changed the chains: a node, a cable, an input?
    Chains,
    /// Could this have changed `/surfaces`?
    Surfaces,
    /// Could this have brought audio to prewarm?
    Audio,
}

/// `<folder>/<name>.k7x` for one of `folders`, as the page's pattern read it: the dot before `k7x` is any character, and the case is exact.
fn is_named_doc(path: &str, folders: &[&str]) -> bool {
    let Some(rest) = folders
        .iter()
        .find_map(|f| path.strip_prefix(f).and_then(|r| r.strip_prefix('/')))
    else {
        return false;
    };
    let Some(body) = rest.strip_suffix("k7x") else {
        return false;
    };
    let mut chars = body.chars();
    // `[^/]+` then any one character, which is not a line break.
    let Some(last) = chars.next_back() else {
        return false;
    };
    let head = chars.as_str();
    !head.is_empty() && !head.contains('/') && !matches!(last, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Whether a reload that wrote `files` concerns `scope`; no list means "I do not know", which every scope reads as re-ask.
#[must_use]
pub fn moved(scope: Scope, files: Option<&[ChangedFile]>) -> bool {
    let Some(files) = files else { return true };
    files.iter().any(|f| match scope {
        Scope::Tree => f.kind != Change::Edited,
        Scope::Vocabulary => f.kind != Change::Edited || !is_named_doc(&f.path, &["clips", "sequences", "curves"]),
        Scope::Chains => !is_named_doc(&f.path, &["tracks", "clips", "sequences", "curves"]),
        Scope::Surfaces => f.path.starts_with("surfaces/"),
        Scope::Audio => f.path.starts_with("tracks/") || f.path.starts_with("clips/") || !is_doc_file(&f.path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(files: &[(&str, Change)]) -> Option<Vec<ChangedFile>> {
        Some(
            files
                .iter()
                .map(|(p, k)| ChangedFile {
                    path: (*p).to_owned(),
                    kind: *k,
                })
                .collect(),
        )
    }

    fn ask(scope: Scope, files: Option<Vec<ChangedFile>>) -> bool {
        moved(scope, files.as_deref())
    }

    use Change::{Added, Edited, Removed};

    #[test]
    fn the_tree_moves_only_when_a_file_appears_or_goes() {
        assert!(!ask(Scope::Tree, at(&[("clips/a.k7x", Edited)])));
        assert!(!ask(Scope::Tree, at(&[("graph.k7x", Edited)])));
        assert!(ask(Scope::Tree, at(&[("clips/new.k7x", Added)])));
        assert!(ask(Scope::Tree, at(&[("clips/old.k7x", Removed)])));
        assert!(ask(Scope::Tree, None));
        // A command that wrote nothing leaves the surface alone.
        assert!(!ask(Scope::Tree, at(&[])));
    }

    #[test]
    fn the_vocabulary_skips_only_a_rewrite_of_a_clip_a_sequence_or_a_curve() {
        for p in ["clips/audio_1.k7x", "sequences/riff.k7x", "curves/fade.k7x"] {
            assert!(!ask(Scope::Vocabulary, at(&[(p, Edited)])), "{p}");
        }
        assert!(ask(Scope::Vocabulary, at(&[("clips/audio_2.k7x", Added)])));
        assert!(ask(Scope::Vocabulary, at(&[("sequences/riff.k7x", Removed)])));
        for p in [
            "tracks/drums.k7x",
            "graph.k7x",
            "project.k7x",
            "modules/audio/amp/module.k7x",
        ] {
            assert!(ask(Scope::Vocabulary, at(&[(p, Edited)])), "{p}");
        }
        // Anchored at the root: a module may be called `clips`.
        assert!(ask(
            Scope::Vocabulary,
            at(&[("modules/audio/clips/module.k7x", Edited)])
        ));
        // A rack's wav beside a clip file is not a name-only edit.
        assert!(ask(Scope::Vocabulary, at(&[("clips/take.wav", Edited)])));
        assert!(ask(
            Scope::Vocabulary,
            at(&[("clips/a.k7x", Edited), ("tracks/drums.k7x", Edited)])
        ));
        assert!(ask(Scope::Vocabulary, None));
    }

    #[test]
    fn the_chains_skip_an_arrangement_edit() {
        assert!(!ask(
            Scope::Chains,
            at(&[("tracks/gtr.k7x", Edited), ("clips/riff.k7x", Edited)])
        ));
        assert!(ask(Scope::Chains, at(&[("chains/gtr.k7x", Edited)])));
        assert!(ask(Scope::Chains, at(&[("nodes/amp.k7x", Edited)])));
        assert!(ask(Scope::Chains, None));
    }

    #[test]
    fn the_surfaces_move_only_for_a_file_under_surfaces() {
        assert!(ask(Scope::Surfaces, at(&[("surfaces/timeline.k7x", Edited)])));
        assert!(!ask(Scope::Surfaces, at(&[("graph.k7x", Edited)])));
        assert!(!ask(Scope::Surfaces, at(&[("tracks/a.k7x", Added)])));
        assert!(ask(Scope::Surfaces, None));
    }

    #[test]
    fn the_audio_moves_for_a_placement_a_clip_or_a_file_that_is_not_a_document() {
        assert!(ask(Scope::Audio, at(&[("tracks/gtr.k7x", Edited)])));
        assert!(ask(Scope::Audio, at(&[("clips/audio_1.k7x", Added)])));
        assert!(ask(Scope::Audio, at(&[("audio/take_3.wav", Added)])));
        // A knob, a cable, a mute: nothing new to decode.
        for p in ["graph.k7x", "chains/gtr.k7x", "sequences/riff.k7x"] {
            assert!(!ask(Scope::Audio, at(&[(p, Edited)])), "{p}");
        }
        assert!(ask(Scope::Audio, None));
    }
}
