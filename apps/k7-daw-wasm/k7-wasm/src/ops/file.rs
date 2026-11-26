// ops/file.rs — the file.* operations: a buffer judged as the compiler's own reader would judge the file at its path.
// A reader would get wrong that positions are the reader's: lines are 1-based and columns are UTF-16 units, as the editor counts.

use crate::error::WasmError;
use crate::ops::write;
use serde::Deserialize;
use song::files::validate_file;

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ValidateArgs {
    /// The workspace path the text is at: its folder and name say which reader judges it.
    pub path: String,
    pub text: String,
}

/// The file family's requests: `{"op": "file.validate", "args": {…}}`.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "op", content = "args")]
pub enum FileRequest {
    #[serde(rename = "file.validate")]
    Validate(ValidateArgs),
}

pub fn dispatch(request: FileRequest, out: &mut Vec<u8>) -> Result<(), WasmError> {
    match request {
        FileRequest::Validate(a) => write(out, &validate_file(&a.path, &a.text)),
    }
}
