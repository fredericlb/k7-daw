//! logs.rs — `/logs`: the account's ring and the bound machine's log, answered from memory and one bounded ask (2622).
//! What a reader gets wrong: a machine that does not answer is the empty half.

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use axum::response::Response;

use crate::routes::files::service::native::callbacks::BoxFuture;
use k7_file_service::runtime::logs::{Query, answer, machine_half, service_half};
use serde_json::Value;

use super::super::super::{Who, workspace};
use super::{Runtime, Slot};
use crate::routes::files::json_answer;

/// `/logs`' machine asks in flight at once, per account (2624).
pub const ASKS: usize = 2;
/// How long the machine may take to answer `/logs` (2624).
pub const WAIT: Duration = Duration::from_secs(5);

/// Called at each reply the machine's agent sees, which renews the asking job's lease (2631).
pub type Heard = Arc<dyn Fn() + Send + Sync>;

/// The bound machine's jobs agent: 1j's trait stands here until it merges.
pub trait Machine: Send + Sync + 'static {
    /// The reply of type `last` to `request`; `None` with no machine bound, a job's failure as its sentence.
    fn ask(
        &self,
        request: Value,
        last: String,
        heard: Heard,
    ) -> BoxFuture<Result<Option<Value>, String>>;
}

/// The `/logs` route where the account holds a ring, none otherwise.
pub async fn live(
    who: &Who<'_>,
    method: &hyper::Method,
    headers: &hyper::HeaderMap,
    path: &str,
    query: &str,
) -> Option<Response> {
    let r = who.held.runtime.as_ref()?;
    (path == "/logs" && method == hyper::Method::GET).then_some(())?;
    let said = route(r, who, headers, query).await;
    super::report::logs_answered();
    Some(said)
}

/// `GET /logs` after `resolveWorkspace`: its refusals (400, 404) first, then the answer (F6).
pub async fn route(
    r: &Runtime,
    who: &Who<'_>,
    headers: &hyper::HeaderMap,
    query: &str,
) -> Response {
    match workspace(who, headers) {
        Ok(_) => json_answer(200, &r.logs(query).await),
        Err(refusal) => *refusal,
    }
}

/// A repeated key is an array in Express, which `Number` reads as NaN; absent is the only `undefined`.
pub fn param(query: &str, name: &str) -> Option<String> {
    let all: Vec<String> = url::form_urlencoded::parse(query.as_bytes())
        .filter(|(k, _)| k == name)
        .map(|(_, v)| v.into_owned())
        .collect();
    (!all.is_empty()).then(|| all.join(","))
}

impl Runtime {
    /// The answer of `GET /logs?after=&offset=&tail=`.
    pub async fn logs(&self, query: &str) -> Value {
        let q = Query::parse(
            param(query, "after").as_deref(),
            param(query, "offset").as_deref(),
            param(query, "tail").as_deref(),
        );
        let machine = self.machine_log(&q).await;
        let service = service_half(&self.lock().ring, &q);
        self.counters.logs.fetch_add(1, Ordering::Relaxed);
        answer(&machine, service)
    }

    async fn machine_log(&self, q: &Query) -> Value {
        // A panel polling a stalled machine never piles asks: a third is the empty half.
        let Some(_slot) = Slot::take(&self.asks, ASKS) else {
            return machine_half(None);
        };
        let ask = self
            .machine()
            .ask(q.machine_request(), "log".into(), Arc::new(|| {}));
        let reply = tokio::time::timeout(self.wait, ask).await;
        match reply {
            Ok(Ok(Some(reply))) => machine_half(reply.get("raw")),
            _ => machine_half(None),
        }
    }
}
