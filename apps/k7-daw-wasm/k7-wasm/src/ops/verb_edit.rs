// ops/verb_edit.rs — the `edit.*` operations: the rules of the lines the page writes and reads back (export, pending edits, reloads, `+`).
// A reader would get wrong that the page's state lives here: pending edits are handed in and out, never held between calls.

use crate::error::WasmError;
use crate::ops::write;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use verbs::ui::export::{self, ExportAsk};
use verbs::ui::lines;
use verbs::ui::node_insert::{self, GraphNames, PendingInsert, Spawn};
use verbs::ui::pending::{self, ClipFields, PendingEdit};
use verbs::ui::reload::{self, ChangedFile, Scope};
use verbs::ui::writes::{self, VerbClass};

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RestArgs {
    pub rest: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TimelineArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub timeline: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PendingOfArgs {
    pub line: String,
    pub num: f64,
    pub den: f64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PendingClipsArgs {
    pub track: String,
    pub clips: Vec<ClipFields>,
    pub edits: Vec<PendingEdit>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SplitAtArgs {
    pub track: String,
    pub start: f64,
    pub edits: Vec<PendingEdit>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MovedArgs {
    pub scope: Scope,
    /// The files a command wrote; none when the page does not know, which every scope reads as re-ask.
    pub files: Option<Vec<ChangedFile>>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LaneArgs {
    pub graph: GraphNames,
    pub before: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NodeNameArgs {
    pub graph: GraphNames,
    pub module: String,
    pub before: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClassLineArgs {
    pub line: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreateArgs {
    pub kind: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CatalogueRefArgs {
    #[serde(default)]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub catalogue_ref: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WireArgs {
    pub node: String,
    pub at: Option<PendingInsert>,
    pub spawn: Option<Spawn>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FilterOpsArgs {
    pub ops: Vec<String>,
    pub search: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProcessArgs {
    pub clip: String,
    pub chain: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FilesArgs {
    pub files: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlaylistFieldsArgs {
    pub path: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlaylistValuesArgs {
    pub refs: Vec<String>,
    pub path: Vec<String>,
    pub field: String,
}

/// A line as a string, or nothing: the answer of the reads that may find none.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MaybeLine {
    pub line: Option<String>,
}

/// A clip's changed fields, or nothing when a pending edit hides it; a null field is taken away.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipPatches {
    #[cfg_attr(feature = "ts", ts(type = "Array<Record<string, number | boolean | null> | null>"))]
    pub patches: Vec<Option<Map<String, Value>>>,
}

/// The `edit.*` requests.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum EditRequest {
    #[serde(rename = "edit.parse_export_ask")]
    ParseExportAsk(RestArgs),
    #[serde(rename = "edit.tracks_line")]
    TracksLine(TimelineArgs),
    #[serde(rename = "edit.pending_of")]
    PendingOf(PendingOfArgs),
    #[serde(rename = "edit.pending_clips")]
    PendingClips(PendingClipsArgs),
    #[serde(rename = "edit.pending_split_at")]
    PendingSplitAt(SplitAtArgs),
    #[serde(rename = "edit.reload_moved")]
    ReloadMoved(MovedArgs),
    #[serde(rename = "edit.lane_of_arrow")]
    LaneOfArrow(LaneArgs),
    #[serde(rename = "edit.node_name_for")]
    NodeNameFor(NodeNameArgs),
    #[serde(rename = "edit.chain_ref_from_create")]
    ChainRefFromCreate(ClassLineArgs),
    #[serde(rename = "edit.create_for_graph_command")]
    CreateForGraphCommand(CreateArgs),
    #[serde(rename = "edit.node_new_ref_arg")]
    NodeNewRefArg(CatalogueRefArgs),
    #[serde(rename = "edit.wire_command")]
    WireCommand(WireArgs),
    #[serde(rename = "edit.filter_ops")]
    FilterOps(FilterOpsArgs),
    #[serde(rename = "edit.process_line")]
    ProcessLine(ProcessArgs),
    #[serde(rename = "edit.playlist_refs")]
    PlaylistRefs(FilesArgs),
    #[serde(rename = "edit.playlist_fields_at")]
    PlaylistFieldsAt(PlaylistFieldsArgs),
    #[serde(rename = "edit.playlist_values_at")]
    PlaylistValuesAt(PlaylistValuesArgs),
    #[serde(rename = "edit.class_of_line")]
    ClassOfLine(ClassLineArgs),
    #[serde(rename = "edit.classed_verbs")]
    ClassedVerbs,
}

pub fn dispatch(request: EditRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        EditRequest::ParseExportAsk(a) => {
            let ask: Option<ExportAsk> = export::parse_export_ask(&a.rest);
            write(out, &ask)
        }
        EditRequest::TracksLine(a) => write(out, &export::tracks_line(a.timeline.as_deref())),
        EditRequest::PendingOf(a) => {
            let edits: Vec<PendingEdit> = pending::pending_of(&a.line, a.num, a.den);
            write(out, &edits)
        }
        EditRequest::PendingClips(a) => write(
            out,
            &ClipPatches {
                patches: pending::pending_clips(&a.track, &a.clips, &a.edits),
            },
        ),
        EditRequest::PendingSplitAt(a) => write(out, &pending::pending_split_at(&a.track, a.start, &a.edits)),
        EditRequest::ReloadMoved(a) => write(out, &reload::moved(a.scope, a.files.as_deref())),
        EditRequest::LaneOfArrow(a) => write(out, &node_insert::lane_of_arrow(&a.graph, a.before.as_deref())),
        EditRequest::NodeNameFor(a) => write(
            out,
            &node_insert::node_name_for(&a.graph, &a.module, a.before.as_deref()),
        ),
        EditRequest::ChainRefFromCreate(a) => write(out, &node_insert::chain_ref_from_create(&a.line)),
        EditRequest::CreateForGraphCommand(a) => write(out, &node_insert::create_for_graph_command(&a.kind, &a.name)),
        EditRequest::NodeNewRefArg(a) => write(out, &node_insert::node_new_ref_arg(a.catalogue_ref.as_deref())),
        EditRequest::WireCommand(a) => write(out, &node_insert::wire_command(&a.node, a.at.as_ref(), a.spawn)),
        EditRequest::FilterOps(a) => write(out, &lines::filter_ops(&a.ops, &a.search)),
        EditRequest::ProcessLine(a) => write(out, &lines::process_line(&a.clip, &a.chain)),
        EditRequest::PlaylistRefs(a) => write(out, &lines::playlist_refs(&a.files)),
        EditRequest::PlaylistFieldsAt(a) => write(out, &lines::playlist_fields_at(&a.path)),
        EditRequest::PlaylistValuesAt(a) => write(out, &lines::playlist_values_at(&a.refs, &a.path, &a.field)),
        EditRequest::ClassOfLine(a) => {
            let class: VerbClass = writes::class_of_line(&a.line);
            write(out, &class)
        }
        EditRequest::ClassedVerbs => write(out, &writes::classed_verbs()),
    }
}
