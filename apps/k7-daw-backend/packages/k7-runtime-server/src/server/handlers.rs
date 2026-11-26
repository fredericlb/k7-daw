//! handlers.rs — the table from a message's `type` to the code that answers it: one line per module in `table`,
//! so a milestone adds its verbs by adding its line and nothing here changes.
//! What a reader gets wrong: a handler runs on its socket's own task, so it must not block; work for the
//! message thread is posted (`Engine::post`), and anything that waits is spawned.

use super::ctx::Request;
use super::msg::Malformed;
use super::whoami;
use std::collections::HashMap;
use std::sync::Arc;

/// One message kind's answer; an `Err` is answered `malformed message: …` and the socket stays.
pub type Handler = Arc<dyn Fn(&Request) -> Result<(), Malformed> + Send + Sync>;

#[derive(Default)]
pub struct HandlerTable(HashMap<String, Handler>);

impl HandlerTable {
    pub fn add(
        &mut self,
        types: &[&str],
        handler: impl Fn(&Request) -> Result<(), Malformed> + Send + Sync + 'static,
    ) {
        let handler: Handler = Arc::new(handler);
        for t in types {
            self.0.insert((*t).to_string(), handler.clone());
        }
    }

    pub fn find(&self, ty: &str) -> Option<&Handler> {
        self.0.get(ty)
    }
}

/// Every handler the server has: each milestone's module adds one line.
pub fn table() -> HandlerTable {
    let mut t = HandlerTable::default();
    whoami::add(&mut t);
    super::project::add(&mut t);
    super::transport::add(&mut t);
    t
}
