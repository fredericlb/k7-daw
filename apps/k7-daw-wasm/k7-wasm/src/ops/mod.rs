// ops/mod.rs — the dispatch: JSON bytes in, JSON bytes out, routed by the op's family to the file that owns it.
// A reader would get wrong that one enum lists every op: each family declares its own, so a new family touches one match arm here.
// A second reader mistake: an answer is the operation's value itself; only an error has a wrapper, and the status says which.

pub mod curve;
pub mod engine;
pub mod fields;
pub mod file;
pub mod k7x;
pub mod k7x_edit;
pub mod schema;
pub mod song;
pub mod timecode;
#[cfg(feature = "verbs")]
pub mod verb;

use crate::error::WasmError;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// The status of a call that answered its value.
pub const OK: u32 = 0;
/// The status of a call that answered a `WasmError`.
pub const ERROR: u32 = 1;

/// What `version` answers.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct VersionAnswer {
    pub sources: String,
}

/// The sources hash `buildWasm.mjs` sets in the build's environment, or `dev` for a build made by hand.
const SOURCES: &str = match option_env!("K7_WASM_SOURCES") {
    Some(s) => s,
    None => "dev",
};

/// The op's name alone, read before the family decodes the whole request.
#[derive(Deserialize)]
struct Op {
    op: String,
}

/// What outlives a call: each family's table of open handles.
#[derive(Default)]
pub struct Session {
    pub curves: curve::CurveTable,
    pub songs: song::SongTable,
    /// The k7x reads' arena and the last outline's layout.
    pub k7x: k7x::K7xState,
}

pub(crate) fn write<T: Serialize + ?Sized>(out: &mut Vec<u8>, value: &T) -> Result<(), WasmError> {
    serde_json::to_writer(out, value).map_err(|e| WasmError::bad_request(e.to_string()))
}

/// A family's request, decoded from the whole input: `{"op": "<family>.<name>", "args": {…}}`.
fn decode<T: DeserializeOwned>(input: &[u8]) -> Result<T, WasmError> {
    serde_json::from_slice(input).map_err(|e| WasmError::bad_request(e.to_string()))
}

impl Session {
    pub fn new() -> Session {
        Session::default()
    }

    /// Runs one request, leaving the answer's JSON in `out`.
    pub fn call(&mut self, input: &[u8], out: &mut Vec<u8>) -> Result<(), WasmError> {
        out.clear();
        let Op { op } = decode(input)?;
        let family = op.split_once('.').map_or(op.as_str(), |(f, _)| f);
        match family {
            "curve" => curve::dispatch(&mut self.curves, decode(input)?, out),
            "engine" | "text" => engine::dispatch(decode(input)?, out),
            "schema" => schema::dispatch(decode(input)?, out),
            "timecode" => timecode::dispatch(decode(input)?, out),
            "fields" => fields::dispatch(decode(input)?, out),
            "file" => file::dispatch(decode(input)?, out),
            "song" => song::dispatch(&mut self.songs, decode(input)?, out),
            "k7x" => k7x::dispatch(&mut self.k7x, decode(input)?, out),
            #[cfg(feature = "verbs")]
            "verb" => verb::dispatch(decode(input)?, out),
            "version" => write(
                out,
                &VersionAnswer {
                    sources: SOURCES.to_owned(),
                },
            ),
            _ => Err(WasmError::bad_request(format!("unknown op '{op}'"))),
        }
    }

    /// Runs one request and answers its status: the value's JSON in `out` for OK, a `WasmError`'s for ERROR.
    pub fn handle(&mut self, input: &[u8], out: &mut Vec<u8>) -> u32 {
        match self.call(input, out) {
            Ok(()) => OK,
            Err(e) => {
                out.clear();
                // A WasmError is two plain fields: serializing it cannot fail, and an empty answer is the fallback.
                let _ = serde_json::to_writer(&mut *out, &e);
                ERROR
            }
        }
    }
}
