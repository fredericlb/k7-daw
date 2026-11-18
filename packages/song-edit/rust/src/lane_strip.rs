// lane_strip.rs — every lane's STRIP, sound and picture alike: one node LAST in the lane's run, before the sum it ends on.
// A reader would get wrong what is here: the names, the strip's file and the plain write into a graph.k7x; the plan and the chain arm are `lane_strip_plan.rs`.

use crate::error::Res;
use crate::fs::Fs;
use crate::graph_file::{Segments, connect_nodes, insert_between, merge_segments};
use crate::graph_schema::GRAPH_FILE;
use crate::node_files::NODES_DIR;
use crate::paths::join;
use crate::workspace_doc::{ws_text, ws_write_text};
use k7x::edit::{Document, Scope, Value};

pub const MIX_NODE: &str = "MIX";
pub const RENDER_NODE: &str = "RENDER";
pub const SHARED_STRIP: &str = "@meta/audio/mixer";
pub const PICTURE_STRIP: &str = "@meta/shaders/video-mixer";

/// What a lane's strip IS: heard through the Mixer, or drawn through the video mixer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripKind {
    Audio,
    Picture,
}

impl StripKind {
    /// The module the strip's node extends.
    #[must_use]
    pub fn reference(self) -> &'static str {
        match self {
            StripKind::Audio => SHARED_STRIP,
            StripKind::Picture => PICTURE_STRIP,
        }
    }

    /// The sum the lane's run ends on.
    #[must_use]
    pub fn sink(self) -> &'static str {
        match self {
            StripKind::Audio => MIX_NODE,
            StripKind::Picture => RENDER_NODE,
        }
    }

    fn holds(self) -> &'static str {
        match self {
            StripKind::Audio => "volume, pan and three bands",
            StripKind::Picture => "red, green, blue and transparency",
        }
    }
}

/// What a lane needs to get its strip: the module to write, and the node it goes after.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StripPlan {
    pub lane: String,
    pub kind: StripKind,
    /// The node name, which is also its folder: one flat key space.
    pub name: String,
    pub rel: String,
    /// The node the strip goes after, or None when the lane is an ORPHAN and arrives wired with it.
    pub after: Option<String>,
}

/// The strip's own file: a NODE on k7's shipped strip, never a copy of it.
#[must_use]
pub fn strip_node_text(kind: StripKind, lane: &str) -> String {
    let reference = serde_quote(kind.reference());
    format!(
        "// {lane}'s channel strip: {}.\n//\n// This is where the lane's level and look are written — its card in the graph, its row in the\n// per-track view and the Mixer view are drawings of this one node.\nextends({reference})\n",
        kind.holds()
    )
}

fn serde_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// `mix-<lane>`, then `-2`, `-3`…: the name has to be free across the project's ONE key space.
#[must_use]
pub fn mixer_name_for(lane: &str, taken: &dyn Fn(&str) -> bool) -> String {
    let base = format!("mix-{lane}");
    if !taken(&base) {
        return base;
    }
    let mut n = 2;
    while taken(&format!("{base}-{n}")) {
        n += 1;
    }
    format!("{base}-{n}")
}

/// A name `mixer_name_for` made for `lane`, so it may follow the lane's on a rename.
#[must_use]
pub fn is_lane_mixer_name(name: &str, lane: &str) -> bool {
    let base = format!("mix-{lane}");
    name == base
        || name
            .strip_prefix(&format!("{base}-"))
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// The strip on the arrow into the sum, or hung on a run that led nowhere and wired on to it.
///
/// # Errors
/// What the graph writers fail with.
pub fn splice_strip(graph: &mut Scope, after: &str, name: &str, sink: &str) -> Res<()> {
    let dests = match graph
        .to_js()?
        .as_ref()
        .and_then(|v| v.get("modules"))
        .and_then(|m| m.get(after))
    {
        Some(Value::List(l)) => l.clone(),
        _ => vec![],
    };
    if dests.contains(&Value::Str(sink.to_owned())) {
        return insert_between(graph, after, Some(sink), name);
    }
    insert_between(graph, after, None, name)?;
    connect_nodes(graph, name, sink)
}

/// The writes a plan implies in a project of segments: a node file and an arrow in graph.k7x.
///
/// # Errors
/// What the graph writers or the file system fail with.
pub fn apply_lane_strip_plain(fs: &mut dyn Fs, dir: &str, plan: &StripPlan) -> Res<Vec<String>> {
    let sink = plan.kind.sink();
    let graph_path = join(&[dir, GRAPH_FILE]);
    // A project with no graph.k7x yet gets one: the lane needs none, but its strip has to be wired.
    let mut doc = Document::new(&if fs.exists(&graph_path) {
        ws_text(fs, &graph_path)?
    } else {
        String::new()
    });
    match &plan.after {
        Some(after) => splice_strip(&mut doc.at(&[]), after, &plan.name, sink)?,
        // An orphan lane is not in the graph at all: it and its strip arrive together.
        None => {
            let modules = vec![
                (plan.lane.clone(), vec![plan.name.clone()]),
                (plan.name.clone(), vec![sink.to_owned()]),
            ];
            merge_segments(
                &mut doc.at(&[]),
                &Segments {
                    modules,
                    ..Segments::default()
                },
            )?;
        }
    }
    let nodes = join(&[dir, NODES_DIR]);
    fs.mkdir_all(&nodes)
        .map_err(|e| crate::error::io_error(&e, "mkdir", &nodes))?;
    ws_write_text(fs, &join(&[dir, &plan.rel]), &strip_node_text(plan.kind, &plan.lane))?;
    ws_write_text(fs, &graph_path, doc.text())?;
    Ok(vec![plan.rel.clone(), GRAPH_FILE.to_owned()])
}
