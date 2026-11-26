//! absent.rs — the session on a platform ONNX Runtime ships no binary for: every open says so.

use std::path::Path;

pub struct OnnxSession;

impl OnnxSession {
    pub fn open(_model: &Path, _providers: &[String]) -> Result<OnnxSession, String> {
        Err(
            "this supervisor was built without ONNX Runtime: it ships no binary for this platform"
                .into(),
        )
    }
    pub fn run(&self, _mix: &[f32]) -> Result<Vec<f32>, String> {
        Err("no ONNX Runtime".into())
    }
    pub fn terminate(&self) {}
    pub fn provider(&self) -> &str {
        ""
    }
    pub fn adapter(&self) -> &str {
        ""
    }
    pub fn skipped(&self) -> &str {
        ""
    }
}

pub fn available() -> bool {
    false
}
