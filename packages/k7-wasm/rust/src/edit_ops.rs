// edit_ops.rs — the edit.* operations: `song_edit::facade` run over a file system, the host's imports in the module.
// A reader would get wrong that a throw is an answer of its own kind (`edit_failed`), whose message is the TypeScript's.

use crate::error::{ErrorCode, WasmError};
use crate::host_fs::HostFs;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use song_edit::Fs;
use song_edit::facade::{self, Call, Ctx, Failure};

#[derive(Clone, Debug, Deserialize)]
pub struct CallArgs {
    /// The TypeScript's own name: `song-edit/trackMix.editTrackStruct`.
    #[serde(rename = "fn")]
    pub name: String,
    #[serde(default)]
    pub args: Vec<Value>,
    /// `{studioHome, metaRoot, ...}`: what the TypeScript read from the process.
    #[serde(default)]
    pub env: Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct BatchArgs {
    pub calls: Vec<CallArgs>,
    #[serde(default)]
    pub env: Value,
}

#[derive(Debug, Serialize)]
pub struct CallAnswer {
    pub ret: Value,
}

/// The results of the calls that ran, and the throw that stopped the batch, if any.
#[derive(Debug, Serialize)]
pub struct BatchAnswer {
    pub rets: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thrown: Option<String>,
}

fn sentence(f: &Failure, name: &str) -> String {
    match f {
        Failure::Thrown(m) => m.clone(),
        Failure::Unsupported(why) => format!("{name}: no such edit function ({why})"),
    }
}

/// One call over `fs`.
pub fn call_over(fs: &mut dyn Fs, new_id: &mut dyn FnMut() -> String, a: &CallArgs) -> Result<CallAnswer, String> {
    let env = facade::env_from_json(&a.env);
    let mut cx = Ctx { fs, env: &env, new_id };
    facade::run_named(&mut cx, &a.name, &a.args)
        .map(|ret| CallAnswer { ret })
        .map_err(|f| sentence(&f, &a.name))
}

/// The calls in order over `fs`, stopping at the first throw.
pub fn batch_over(fs: &mut dyn Fs, new_id: &mut dyn FnMut() -> String, a: &BatchArgs) -> BatchAnswer {
    let env = facade::env_from_json(&a.env);
    let mut cx = Ctx { fs, env: &env, new_id };
    let calls: Vec<Call> = a
        .calls
        .iter()
        .map(|c| Call {
            name: c.name.clone(),
            args: c.args.clone(),
        })
        .collect();
    let b = facade::batch(&mut cx, &calls);
    let thrown = b.failed.map(|f| {
        let name = calls.get(b.done.len()).map_or("", |c| c.name.as_str());
        sentence(&f, name)
    });
    BatchAnswer { rets: b.done, thrown }
}

fn id() -> String {
    crate::abi::new_id()
}

/// A call over the host's file system; a path it lacked is a `needs_files` answer, whatever the call said.
pub fn call(a: CallArgs) -> Result<CallAnswer, WasmError> {
    let mut fs = HostFs::new();
    let r = call_over(&mut fs, &mut id, &a);
    let missing = fs.take_missing();
    if !missing.is_empty() {
        return Err(WasmError::needs_files(missing));
    }
    r.map_err(|m| WasmError::new(ErrorCode::EditFailed, m))
}

pub fn batch(a: BatchArgs) -> Result<BatchAnswer, WasmError> {
    let mut fs = HostFs::new();
    let r = batch_over(&mut fs, &mut id, &a);
    let missing = fs.take_missing();
    if !missing.is_empty() {
        return Err(WasmError::needs_files(missing));
    }
    Ok(r)
}
