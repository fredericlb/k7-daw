// api.rs — a verb as JSON: `{line, project, env, answers?}` in, `{log, refuse, refused, effects, asks?, pending?, threw?}` out, over the caller's `Fs`.
// A reader would get wrong where the change set is: in the `Fs`, whose writes the caller lands whole, or drops when the run is refused or pending.

use crate::host::Host;
use crate::run::run_line;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use song::Cache;
use song_edit::Fs;
use song_edit::confine::Env;
use song_edit::facade::env_from_json;

#[derive(Clone, Debug, Deserialize)]
pub struct VerbRequest {
    /// The command line as typed: `event move drums 2:1 3:1`.
    pub line: String,
    /// The project folder, in the `Fs`'s spelling.
    pub project: String,
    /// `{studioHome, metaRoot, ...}`, as `edit.call` takes it.
    #[serde(default)]
    pub env: Json,
    /// The answers of the routes earlier runs of this line called, in order (`Host::call`).
    #[serde(default)]
    pub answers: Vec<Json>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    pub log: Vec<String>,
    pub refuse: Vec<String>,
    /// Whether the verb said no; its writes, if any, must not land.
    pub refused: bool,
    /// What the caller performs after landing the writes, in order: `reload`, `applyProject`.
    pub effects: Vec<String>,
    /// The host members the verb asks for, in order (`Host::ask`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asks: Vec<Json>,
    /// The route the run stopped at, `{route, args}`: the caller asks it and runs the line again with its answer; nothing lands.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<Json>,
    /// A throw that escaped the verb: its message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub threw: Option<String>,
}

/// Runs the request's line over `fs`, fresh ids from `new_id`.
pub fn run_request(fs: &mut dyn Fs, new_id: &mut dyn FnMut() -> String, req: &VerbRequest) -> Answer {
    run_request_cached(fs, &mut Cache::new(), new_id, req)
}

/// `run_request` whose compiles reuse `cache`, which a caller keeps from one line to the next.
pub fn run_request_cached(
    fs: &mut dyn Fs,
    cache: &mut Cache,
    new_id: &mut dyn FnMut() -> String,
    req: &VerbRequest,
) -> Answer {
    let env = env_from_json(&req.env);
    run_answered(fs, cache, new_id, env, &req.project, &req.line, req.answers.clone())
}

/// One line over the project `project` with the roots `env`, as a native caller that holds an `Env` already runs it.
pub fn run_in_env(
    fs: &mut dyn Fs,
    cache: &mut Cache,
    new_id: &mut dyn FnMut() -> String,
    env: Env,
    project: &str,
    line: &str,
) -> Answer {
    run_answered(fs, cache, new_id, env, project, line, Vec::new())
}

/// `run_in_env` given the answers of the routes its earlier runs called.
pub fn run_answered(
    fs: &mut dyn Fs,
    cache: &mut Cache,
    new_id: &mut dyn FnMut() -> String,
    env: Env,
    project: &str,
    line: &str,
    answers: Vec<Json>,
) -> Answer {
    let mut h = Host::new(fs, env, project, new_id);
    h.answers = answers;
    std::mem::swap(&mut h.cache, cache);
    let threw = run_line(&mut h, line).err().map(|t| t.0);
    std::mem::swap(&mut h.cache, cache);
    // A pending run says nothing yet, whatever the verb did with the throw: its lines come with the run that has every answer.
    if let Some(pending) = h.pending {
        return Answer {
            pending: Some(pending),
            ..Answer::default()
        };
    }
    Answer {
        refused: !h.refused.is_empty() || threw.is_some(),
        log: h.log,
        refuse: h.refused,
        effects: h.effects.iter().map(|e| (*e).to_owned()).collect(),
        asks: h.asks,
        pending: None,
        threw,
    }
}
