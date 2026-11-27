// error.rs — what a call answers when it fails: a code a caller branches on, and a message a person may read.
// A reader would get wrong that `panic` and `unavailable` are never raised here: the TypeScript side raises them.

use serde::Serialize;
use song_schema::SchemaError;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The JSON does not decode, the op is unknown, or an argument is out of range.
    BadRequest,
    /// A timecode that does not parse.
    BadTimecode,
    BadCurve,
    UnknownHandle,
    /// The instance trapped and was replaced.
    Panic,
    /// No WebAssembly in this runtime.
    Unavailable,
    /// A read reached a file the bundle does not hold: `files` names them, and the caller fetches and asks again.
    NeedsFiles,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct WasmError {
    pub code: ErrorCode,
    pub message: String,
    /// The paths of a `needs_files` answer.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub files: Vec<String>,
}

impl WasmError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> WasmError {
        WasmError {
            code,
            message: message.into(),
            files: Vec::new(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> WasmError {
        WasmError::new(ErrorCode::BadRequest, message)
    }
}

impl From<SchemaError> for WasmError {
    fn from(e: SchemaError) -> WasmError {
        match e {
            SchemaError::BadTimecode(m) => WasmError::new(ErrorCode::BadTimecode, m),
            SchemaError::BadCurve(m) => WasmError::new(ErrorCode::BadCurve, m),
        }
    }
}

impl WasmError {
    /// A read the bundle cannot answer, naming the paths the caller must fetch.
    pub fn needs_files(files: Vec<String>) -> WasmError {
        WasmError {
            code: ErrorCode::NeedsFiles,
            message: "a file the compile reads is not in the bundle".to_owned(),
            files,
        }
    }
}
