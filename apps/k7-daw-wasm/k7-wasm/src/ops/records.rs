// ops/records.rs — the records.* operations: a set list's lines against the workspace's audio files, and the names a row shows.
// A reader would get wrong that `files` is a list crossing whole: a null one is a tree not read yet, which holds nothing.

use crate::error::WasmError;
use crate::ops::schema::PlaylistItem;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use song_edit::records::{
    TreeNode as Node, audio_files_of, is_record_ref, record_folder, record_matches, record_name, record_path_of,
    resolve_record,
};
use std::collections::HashSet;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LineArgs {
    pub line: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathArgs {
    pub path: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MatchArgs {
    pub path: String,
    pub query: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResolveArgs {
    pub items: Vec<PlaylistItem>,
    #[serde(default)]
    pub files: Option<Vec<String>>,
}

/// The shape of a tree node the audio walk reads: a workspace path, a folder or not, and a folder's children.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[cfg_attr(feature = "ts", ts(rename = "TreeNode"))]
pub struct TreeNodeDto {
    pub path: String,
    pub dir: bool,
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub children: Option<Vec<TreeNodeDto>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct AudioFilesArgs {
    pub tree: Vec<TreeNodeDto>,
    pub with_stems: bool,
}

/// One line of a set list, as far as the workspace's files take it.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ResolvedRecord {
    pub item: PlaylistItem,
    /// The record's workspace path, or null for a line that names none.
    pub path: Option<String>,
    /// Whether that audio file is in the workspace.
    pub present: bool,
    /// Whether its four stems are beside it.
    pub stems: bool,
}

/// The records family's requests: `{"op": "records.path_of", "args": {"line": "records/a.flac"}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum RecordsRequest {
    #[serde(rename = "records.path_of")]
    PathOf(LineArgs),
    #[serde(rename = "records.is_ref")]
    IsRef(LineArgs),
    #[serde(rename = "records.name")]
    Name(PathArgs),
    #[serde(rename = "records.folder")]
    Folder(PathArgs),
    #[serde(rename = "records.matches")]
    Matches(MatchArgs),
    #[serde(rename = "records.resolve")]
    Resolve(ResolveArgs),
    #[serde(rename = "records.audio_files_of")]
    AudioFilesOf(AudioFilesArgs),
}

fn node(n: TreeNodeDto) -> Node {
    Node {
        path: n.path,
        dir: n.dir,
        children: n.children.unwrap_or_default().into_iter().map(node).collect(),
    }
}

pub fn dispatch(request: RecordsRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    use RecordsRequest as R;
    match request {
        R::PathOf(a) => write(out, &record_path_of(&a.line)),
        R::IsRef(a) => write(out, &is_record_ref(&a.line)),
        R::Name(a) => write(out, record_name(&a.path)),
        R::Folder(a) => write(out, &record_folder(&a.path)),
        R::Matches(a) => write(out, &record_matches(&a.path, &a.query)),
        R::Resolve(a) => {
            let set: Option<HashSet<&str>> = a.files.as_ref().map(|f| f.iter().map(String::as_str).collect());
            let rows: Vec<ResolvedRecord> = a
                .items
                .iter()
                .map(|item| {
                    let r = resolve_record(&item.song, set.as_ref());
                    ResolvedRecord {
                        item: item.clone(),
                        path: r.path,
                        present: r.present,
                        stems: r.stems,
                    }
                })
                .collect();
            write(out, &rows)
        }
        R::AudioFilesOf(a) => {
            let tree: Vec<Node> = a.tree.into_iter().map(node).collect();
            write(out, &audio_files_of(&tree, a.with_stems))
        }
    }
}
