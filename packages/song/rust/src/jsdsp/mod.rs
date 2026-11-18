// jsdsp/mod.rs — a DSP module's graph: a `dsp.k7x` decoded on every read, or a legacy `jsdsp.json` read and never rebuilt.
// A reader would get wrong that a bad graph stops anything: it plays as a pass-through and the state says why (decision 350).
// Nothing runs a module's code any more; docs/dsp-literal-plan.md and docs/graphModel.md, "`jsdsp` and Faust".

mod validate;

pub use validate::{NODE_TYPES, validate_graph};

use crate::dsp_literal::decode_reason;
use crate::source::{Kind, Source, SourceError, Span};
use std::sync::Arc;

/// What a graph that cannot play is replaced by: a valid graph with no node, which both engine hosts pass through.
pub const PASS_THROUGH_GRAPH: &str = "{\"params\":[],\"nodes\":[]}\n";

/// A graph larger than this is not read, and the engine refuses it too.
pub const MAX_GRAPH: u64 = 1 << 20;

/// How the graph of a jsdsp module stands against its source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphState {
    Fresh,
    /// No graph to play: the pass-through stands in.
    Missing,
    /// The graph is there and cannot be read; the sentence says why, without the file's words.
    Invalid(String),
    /// A legacy `jsdsp.json`, which plays with no word: nothing builds a `jsdsp.ts` any more.
    Orphan,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JsDspGraph {
    /// The graph text the engine receives, unchanged.
    pub text: Arc<str>,
    pub state: GraphState,
    pub json_path: String,
}

/// The cache file of a legacy jsdsp source: `.ts`, `.tsx`, `.js` or `.jsx` dropped, then `.json` (decision 349).
pub fn cache_of(source: &str) -> String {
    let base = [".tsx", ".jsx", ".ts", ".js"]
        .iter()
        .find_map(|ext| source.strip_suffix(ext))
        .unwrap_or(source);
    format!("{base}.json")
}

/// What reading a graph's file found: its text, or why it is no graph.
enum Read {
    Absent,
    Refused(String),
    Text(String),
}

fn refused(what: &str, why: &str) -> Read {
    Read::Refused(format!("its {what} {why}"))
}

fn read_text(source: &dyn Source, path: &str, what: &str) -> Result<Read, SourceError> {
    let st = match source.stat(path) {
        Ok(st) => st,
        Err(SourceError::NotFound) => return Ok(Read::Absent),
        Err(SourceError::Missing) => return Err(SourceError::Missing),
        Err(SourceError::Outside) => return Ok(refused(what, "lies outside the project's folders")),
        Err(SourceError::Io(_)) => return Ok(refused(what, "cannot be read")),
    };
    if st.kind != Kind::File {
        return Ok(refused(what, "is a folder"));
    }
    if st.len > MAX_GRAPH {
        return Ok(refused(what, "is over 1 MiB"));
    }
    let bytes = match source.read(path, Span::All) {
        Ok(b) => b,
        Err(SourceError::NotFound) => return Ok(Read::Absent),
        // A bundle holds only UTF-8, so a file it stat'ed and cannot read is one that is not.
        Err(SourceError::Missing) => return Ok(refused(what, "is not valid UTF-8")),
        Err(_) => return Ok(refused(what, "cannot be read")),
    };
    if bytes.len() as u64 > MAX_GRAPH {
        return Ok(refused(what, "is over 1 MiB"));
    }
    Ok(match String::from_utf8(bytes) {
        Ok(text) => Read::Text(text),
        Err(_) => refused(what, "is not valid UTF-8"),
    })
}

/// The graph of the DSP source at `path`: a `dsp.k7x` decoded, any other path a legacy `jsdsp.ts`'s committed cache (decisions 350 to 353, 1901).
pub fn jsdsp_graph(source: &dyn Source, path: &str) -> Result<JsDspGraph, SourceError> {
    let literal = path.ends_with(".k7x");
    let json_path = if literal { path.to_owned() } else { cache_of(path) };
    let what = if literal { "dsp.k7x" } else { "jsdsp.json" };
    let pass = |state| JsDspGraph {
        text: Arc::from(PASS_THROUGH_GRAPH),
        state,
        json_path: json_path.clone(),
    };
    let text = match read_text(source, &json_path, what)? {
        Read::Absent => return Ok(pass(GraphState::Missing)),
        Read::Refused(why) => return Ok(pass(GraphState::Invalid(why))),
        Read::Text(text) => text,
    };
    let (graph, state) = if literal {
        match decode_reason(path, &text) {
            Ok(graph) => (graph, GraphState::Fresh),
            Err(why) => return Ok(pass(GraphState::Invalid(why))),
        }
    } else {
        (text, GraphState::Orphan)
    };
    Ok(match validate_graph(&graph) {
        Ok(()) => JsDspGraph {
            text: Arc::from(graph),
            state,
            json_path,
        },
        Err(why) => pass(GraphState::Invalid(why)),
    })
}
