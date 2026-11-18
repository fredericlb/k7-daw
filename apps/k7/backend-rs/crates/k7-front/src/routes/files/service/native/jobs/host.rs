//! host.rs — the calls the jobs are asked (`jobs.*`, `render.*`, `separate.*`, 1k): answered on the runtime's
//! thread from memory, or handed to the driver that K4 supplies. What a reader gets wrong: with no driver nothing renders and
//! nothing is cut, and the answers are the TypeScript's own when no machine is bound; a query never waits on the runtime (2351).

use std::time::Instant;

use crate::routes::files::service::native::callbacks::Reply;
use k7_file_service::jobs::separations::separation_id;
use serde_json::{Value, json};

use super::Jobs;

fn str_of(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{key}: a string is needed"))
}

fn strings(args: &Value, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

impl Jobs {
    /// Whether some machine can render: the host route's refusal before it starts a job.
    pub fn renderer_available(&self) -> bool {
        self.driver.as_ref().is_some_and(|d| d.renderer_available())
    }

    /// Whether a machine's jobs path is bound, which a cut needs.
    pub fn bound(&self) -> bool {
        self.driver.as_ref().is_some_and(|d| d.bound())
    }

    /// A tell or a query of the stand-ins; the answer is `{"value": …}` or `{}`.
    pub fn query(&self, kind: &str, args: &Value) -> Result<Value, String> {
        let value = |v: Value| Ok(json!({ "value": v }));
        match kind {
            "jobs.thawed" => value(json!(self.thawed.of(&str_of(args, "dir")?))),
            "jobs.setThawed" => {
                self.set_thawed(&str_of(args, "dir")?, strings(args, "modules"));
                Ok(json!({}))
            }
            "jobs.thaw" => {
                let on = args.get("on").and_then(Value::as_bool).unwrap_or(false);
                self.thaw(&str_of(args, "dir")?, &str_of(args, "module")?, on);
                Ok(json!({}))
            }
            "render.available" => value(json!(
                self.driver.as_ref().is_some_and(|d| d.renderer_available())
            )),
            "separate.job" => {
                let job = self.registry().job(&str_of(args, "id")?);
                value(job.map_or(Value::Null, Value::Object))
            }
            "separate.start" => self.separate(args),
            other => Err(format!("{other}: not a call the jobs answer")),
        }
    }

    /// The registry's `start`: the record at once, the request to the machine sent by the driver.
    fn separate(&self, args: &Value) -> Result<Value, String> {
        let (user, source) = (str_of(args, "userEmail")?, str_of(args, "source")?);
        let model = str_of(args, "model")?;
        let outputs = strings(args, "outputs");
        let id = separation_id(&user, &source, &model, &outputs);
        let bound = self.driver.as_ref().is_some_and(|d| d.bound());
        let started = self.registry().start(Instant::now(), &id, &source, bound);
        if started.send
            && let Some(driver) = &self.driver
        {
            driver.separate(&id, started.generation, args.clone());
        }
        Ok(json!({ "value": started.job }))
    }

    /// An ask of the stand-ins: answered later, from a task of the driver's.
    pub fn ask(&self, kind: &str, args: Value, reply: Reply) {
        let dir = args.get("dir").and_then(Value::as_str).map(str::to_owned);
        let Some(dir) = dir.filter(|_| kind == "jobs.renderAll") else {
            return self.ask_until(kind, args, reply, super::never());
        };
        self.awaited.begin(&dir);
        let guard = super::AwaitedEnd(self.awaited.clone(), dir);
        let reply: Reply = Box::new(move |r| {
            let _guard = guard;
            reply(r)
        });
        self.ask_until(kind, args, reply, super::never())
    }

    /// `ask` for an asker that may give up: a `render.run` is stopped, its child killed, when `gone` resolves; no reply is made.
    pub fn ask_until(&self, kind: &str, args: Value, reply: Reply, gone: super::Gone) {
        let Some(driver) = self.driver.clone() else {
            let none = match kind {
                "render.run" => "no engine machine to render on",
                _ => "the engine jobs have no driver",
            };
            return reply(Err(none.to_owned()));
        };
        match kind {
            "render.run" => {
                self.counts
                    .renders
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                driver.render_run_until(args, reply, gone)
            }
            "jobs.renderAll" => match str_of(&args, "dir") {
                Ok(dir) => driver.render_all(dir, reply),
                Err(e) => reply(Err(e)),
            },
            other => reply(Err(format!("{other}: not an ask the jobs answer"))),
        }
    }
}
