// facade — the crate's call-by-name surface: `call("song-edit/trackMix.addTrackPlacement", args)` and `batch`, JSON in and JSON out.
// A reader would get wrong that the name tables ARE the contract: one entry per TypeScript export the crate answers, and a test names every one.

mod args;
mod base;
mod files;
mod graph;
mod lane;
mod node;
mod prefs;
mod request;
mod time;
mod track;

pub use prefs::prefs_json;
pub use request::{Call, env_from_json};

use crate::Fs;
use crate::confine::Env;
use crate::ids::NewId;
use serde_json::Value as Json;

/// Why a call answered no value: the TypeScript's own thrown message, or a name this surface cannot serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Failure {
    Thrown(String),
    Unsupported(&'static str),
}

pub type Out = Result<Json, Failure>;

/// What a call is given beside its arguments; the crate reads no environment, clock or random source.
pub struct Ctx<'a> {
    pub fs: &'a mut dyn Fs,
    pub env: &'a Env,
    pub new_id: NewId<'a>,
}

/// The side of the crate a name belongs to; a caller that screens arguments screens by it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Base,
    Node,
    Time,
    Lane,
}

type Table = &'static [(&'static str, &'static [&'static str])];
type Runner = fn(&mut Ctx, &str, &str, &[Json]) -> Option<Out>;

const GROUPS: &[(Side, Table, Runner)] = &[
    (Side::Base, base::NAMES, base::run),
    (Side::Node, node::NAMES, node::run),
    (Side::Node, graph::NAMES, graph::run),
    (Side::Time, time::NAMES, time::run),
    (Side::Lane, lane::NAMES, lane::run),
    (Side::Lane, prefs::NAMES, prefs::run),
    (Side::Lane, track::NAMES, track::run),
    (Side::Lane, files::NAMES, files::run),
];

fn listed(table: Table, module: &str, f: &str) -> bool {
    table.iter().any(|(m, fns)| *m == module && fns.contains(&f))
}

/// Every name the surface answers, as `module.fn`.
#[must_use]
pub fn names() -> Vec<String> {
    let mut out = Vec::new();
    for (_, table, _) in GROUPS {
        for (m, fns) in *table {
            out.extend(fns.iter().map(|f| format!("{m}.{f}")));
        }
    }
    out
}

/// The side a name `module.fn` belongs to, None for a name the surface does not know.
#[must_use]
pub fn side_of(name: &str) -> Option<Side> {
    let (module, f) = name.rsplit_once('.')?;
    GROUPS.iter().find(|(_, t, _)| listed(t, module, f)).map(|(s, _, _)| *s)
}

/// Runs the function `module`.`f` with its positional arguments, telling a throw from a name it cannot serve.
pub fn run(cx: &mut Ctx, module: &str, f: &str, args: &[Json]) -> Out {
    for (_, table, run) in GROUPS {
        if listed(table, module, f)
            && let Some(out) = run(cx, module, f, args)
        {
            return out;
        }
    }
    // The rest of trackMix's exports take a callback over a document.
    Err(Failure::Unsupported(if module == "song-edit/trackMix" {
        "a callback"
    } else {
        "no port"
    }))
}

/// `run` for a name spelled `module.fn`.
pub fn run_named(cx: &mut Ctx, name: &str, args: &[Json]) -> Out {
    match name.rsplit_once('.') {
        Some((module, f)) => run(cx, module, f, args),
        None => Err(Failure::Unsupported("no port")),
    }
}

/// The function's value as JSON.
///
/// # Errors
/// The TypeScript's thrown message, or a sentence saying the call has no arm.
pub fn call(cx: &mut Ctx, module: &str, name: &str, args: &[Json]) -> Result<Json, String> {
    run(cx, module, name, args).map_err(|e| match e {
        Failure::Thrown(m) => m,
        Failure::Unsupported(why) => format!("{module}.{name}: no such edit function ({why})"),
    })
}

/// The results of a batch: those that ran, and the failure that stopped it.
#[derive(Debug, PartialEq)]
pub struct Batch {
    pub done: Vec<Json>,
    pub failed: Option<Failure>,
}

/// Runs the calls in order and stops at the first failure.
pub fn batch(cx: &mut Ctx, calls: &[Call]) -> Batch {
    let mut done = Vec::with_capacity(calls.len());
    for c in calls {
        match run_named(cx, &c.name, &c.args) {
            Ok(v) => done.push(v),
            Err(e) => return Batch { done, failed: Some(e) },
        }
    }
    Batch { done, failed: None }
}
