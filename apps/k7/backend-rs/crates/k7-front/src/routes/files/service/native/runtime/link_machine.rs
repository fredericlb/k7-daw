//! link_machine.rs — the bound machine's jobs agent as `Machine`: one socket per ask, closed after its last reply (2660).
//! What a reader gets wrong: no binding is `Ok(None)`, not an error; a job's failure is its sentence.

use serde_json::Value;

use super::logs::{Heard, Machine};
use crate::routes::engine_reach::job::{JOB_IDLE, JOB_TOTAL, JobChannel, request_job};
use crate::routes::files::service::native::callbacks::BoxFuture;
use crate::routes::files::service::native::engine::EngineLink;
use crate::routes::files::service::native::engine::jobs::WsChannel;

/// The account's engine link, which knows the bound machine's jobs path.
pub struct LinkMachine(pub EngineLink);

impl Machine for LinkMachine {
    fn ask(
        &self,
        request: Value,
        last: String,
        heard: Heard,
    ) -> BoxFuture<Result<Option<Value>, String>> {
        let url = self.0.jobs_url();
        Box::pin(async move {
            let Some(url) = url else {
                return Ok(None);
            };
            let body = request.as_object().cloned().unwrap_or_default();
            let mut chan = WsChannel::open(&url).await?;
            let done = request_job(
                &mut chan,
                body,
                |r| r.get("type").and_then(Value::as_str) == Some(last.as_str()),
                |_| heard(),
                JOB_TOTAL,
                JOB_IDLE,
            )
            .await;
            chan.close();
            done.map(|r| Some(Value::Object(r))).map_err(|e| e.0)
        })
    }
}
