//! reply.rs — what a route answers: a status and a JSON body.

use serde_json::Value;

pub enum Body {
    Json(Value),
}

pub struct Reply {
    pub status: u16,
    pub body: Body,
}

impl Reply {
    pub fn json(status: u16, v: Value) -> Reply {
        Reply {
            status,
            body: Body::Json(v),
        }
    }

    /// The JSON body.
    pub fn value(&self) -> Option<&Value> {
        match &self.body {
            Body::Json(v) => Some(v),
        }
    }
}
