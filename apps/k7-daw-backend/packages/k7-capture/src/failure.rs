//! failure.rs — what a history ask refuses with: a status, a word and a sentence for the screen.
//! What a reader gets wrong: no variant carries a secret, and the sentence names what failed, never the request.

use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub struct Failure {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
    /// Fields the screen reads beside the sentence.
    pub extra: Value,
}

impl Failure {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Failure {
        Failure {
            status,
            code,
            message: message.into(),
            extra: Value::Null,
        }
    }

    pub fn body(&self) -> Value {
        let mut b = json!({ "error": self.message, "code": self.code });
        if let (Some(m), Some(x)) = (b.as_object_mut(), self.extra.as_object()) {
            m.extend(x.clone());
        }
        b
    }
}

impl From<std::io::Error> for Failure {
    fn from(e: std::io::Error) -> Failure {
        Failure::new(500, "internal", format!("the workspace's files: {e}"))
    }
}
