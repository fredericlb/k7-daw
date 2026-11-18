//! separate.rs — a cut asked of the bound machine over its jobs path, its record written back into the registry (2330, 2351).
//! What a reader gets wrong: the request runs in a task and `separate.start` has already answered; a request of an older generation writes nothing.

use std::sync::Weak;
use std::sync::atomic::Ordering;
use std::time::Instant;

use serde_json::{Map, Value, json};

use super::Jobs;
use crate::routes::engine_reach::job::{
    JOB_IDLE, JOB_TOTAL, JobChannel, JobError, of_type, request_job,
};
use crate::routes::files::service::native::engine::jobs::WsChannel;

impl Jobs {
    fn separation_progress(&self, id: &str, generation: u64, record: Map<String, Value>) {
        self.registry()
            .progress(Instant::now(), id, generation, record);
    }

    pub(super) fn separation_failed(
        &self,
        id: &str,
        generation: u64,
        from_machine: bool,
        message: &str,
    ) {
        self.registry()
            .fail(Instant::now(), id, generation, from_machine, message);
    }
}

/// The machine's request, from the stand-in's `{userEmail, source, model, outputs, outDir}`.
fn request_of(args: &Value) -> Map<String, Value> {
    let get = |k: &str| args.get(k).cloned().unwrap_or(Value::Null);
    let Value::Object(m) = json!({
        "type": "separate", "user": get("userEmail"), "source": get("source"),
        "model": get("model"), "outputs": get("outputs"), "outDir": get("outDir"),
    }) else {
        unreachable!()
    };
    m
}

/// Asks the machine at `url` for the cut named `id`; every record it sends is the registry's, until it ends.
pub async fn run(jobs: Weak<Jobs>, url: String, id: String, generation: u64, args: Value) {
    let write = |f: &dyn Fn(&Jobs)| {
        if let Some(j) = jobs.upgrade() {
            f(&j);
        }
    };
    let mut chan = match WsChannel::open(&url).await {
        Ok(c) => c,
        Err(e) => return write(&|j| j.separation_failed(&id, generation, false, &e)),
    };
    write(&|j| {
        j.counts.separations.fetch_add(1, Ordering::Relaxed);
    });
    let done = request_job(
        &mut chan,
        request_of(&args),
        of_type("separated"),
        |r| {
            if let Some(Value::Object(rec)) = r.get("job") {
                write(&|j| j.separation_progress(&id, generation, rec.clone()));
            }
        },
        JOB_TOTAL,
        JOB_IDLE,
    )
    .await;
    chan.close();
    if let Err(JobError(e)) = done {
        write(&|j| j.separation_failed(&id, generation, true, &e));
    }
}
