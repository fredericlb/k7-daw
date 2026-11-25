//! verb_ask.rs — a verb's host member asked of the front's own routes: `{route, args}` sent with the caller's headers, a job polled to its end.
//! What a reader gets wrong: the verb speaks real paths and the routes virtual ones, so the args and the answer are mapped both ways here.

use std::time::Duration;

use http_body_util::BodyExt;
use hyper::Method;
use k7_file_service::verb_host::gate::out_dir;
use k7_file_service::verb_host::jobs::Table;
use k7_file_service::verb_paths::Virtual;
use serde_json::{Map, Value, json};

use super::verb_host::now;
use crate::routes::files::service::routes::Reenter;

/// How often a job is asked after, as the Worker's members polled it.
const POLL: Duration = Duration::from_secs(1);

/// What the asks need: the front, the account a job belongs to, the open project's name and the path mapping.
pub struct Asker<'a> {
    pub reenter: Option<&'a Reenter>,
    pub account: &'a str,
    pub studio: &'a str,
    /// The open project as the routes' `at` names it; none with no project open.
    pub at: Option<String>,
    pub virt: Option<&'a Virtual>,
}

/// Every string of `v` that starts with a slash, mapped by `f` where it names a path under a root.
fn map_paths(v: &Value, f: &dyn Fn(&str) -> Option<String>) -> Value {
    match v {
        Value::String(s) if s.starts_with('/') => Value::String(f(s).unwrap_or_else(|| s.clone())),
        Value::Array(a) => Value::Array(a.iter().map(|x| map_paths(x, f)).collect()),
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| (k.clone(), map_paths(x, f)))
                .collect(),
        ),
        _ => v.clone(),
    }
}

/// `{"error", "code"?, "status"}`, the failure a verb words.
fn failed(status: u16, body: &Value, fallback: &str) -> Value {
    let error = body
        .get("error")
        .and_then(Value::as_str)
        .unwrap_or(fallback);
    let mut out = json!({ "error": error, "status": status });
    if let Some(code) = body.get("code").and_then(Value::as_str) {
        out["code"] = json!(code);
    }
    out
}

/// The query a GET's args make: each field, a text as itself and anything else as JSON.
fn query_of(args: &Value) -> String {
    let mut q = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in args.as_object().into_iter().flatten() {
        let text = v.as_str().map_or_else(|| v.to_string(), str::to_owned);
        q.append_pair(k, &text);
    }
    q.finish()
}

impl Asker<'_> {
    fn to_routes(&self, v: &Value) -> Value {
        match self.virt {
            Some(virt) => map_paths(v, &|s| virt.shown(s)),
            None => v.clone(),
        }
    }

    fn to_verb(&self, v: &Value) -> Value {
        match self.virt {
            Some(virt) => map_paths(v, &|s| virt.real(s)),
            None => v.clone(),
        }
    }

    /// One request to the route; its status and JSON body.
    async fn send(&self, route: &str, args: &Value) -> (u16, Value) {
        let Some(reenter) = self.reenter else {
            return (
                503,
                json!({ "error": "this host asks no route", "code": "off" }),
            );
        };
        let (method, path) = route.split_once(' ').unwrap_or(("POST", route));
        let args = self.to_routes(args);
        let (method, target, body) = if method == "GET" {
            let q = query_of(&args);
            let target = if q.is_empty() {
                path.to_owned()
            } else {
                format!("{path}?{q}")
            };
            (Method::GET, target, Value::Null)
        } else {
            let mut body = args.as_object().cloned().unwrap_or_else(Map::new);
            if let Some(at) = &self.at {
                body.entry("at").or_insert_with(|| json!(at));
            }
            (Method::POST, path.to_owned(), Value::Object(body))
        };
        let answered = reenter(method, target, body).await;
        let status = answered.status().as_u16();
        let bytes = answered
            .into_body()
            .collect()
            .await
            .map(|b| b.to_bytes())
            .unwrap_or_default();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    /// The answer a verb's `call` gets: the route's body, a job's value with `job` and `dir`, or `{error, code?, status}`.
    pub async fn call(&self, route: &str, args: &Value) -> Value {
        let (status, body) = self.send(route, args).await;
        if status > 299 {
            return failed(status, &body, &format!("{route} answered {status}"));
        }
        let Some(job) = body
            .get("job")
            .and_then(Value::as_str)
            .filter(|_| status == 202)
        else {
            return self.to_verb(&body);
        };
        let job = job.to_owned();
        loop {
            let state = Table::global()
                .poll(self.account, &job, now())
                .unwrap_or(Value::Null);
            match state.get("state").and_then(Value::as_str) {
                Some("done") => {
                    let mut value = self.to_verb(state.get("value").unwrap_or(&Value::Null));
                    let dir = out_dir(self.studio, &job).unwrap_or_default();
                    if let Some(o) = value.as_object_mut() {
                        o.insert("job".into(), json!(job));
                        o.insert("dir".into(), json!(dir));
                    }
                    return value;
                }
                Some("running") => tokio::time::sleep(POLL).await,
                Some(_) => return failed(500, &state, "the job failed"),
                None => return failed(404, &json!({}), "the job is gone"),
            }
        }
    }

    /// An ask after the landing: its `lines`, or its failure as one line; a job it starts goes on without the verb.
    pub async fn ask(&self, route: &str, args: &Value) -> Vec<String> {
        let (status, body) = self.send(route, args).await;
        if status > 299 {
            let e = failed(status, &body, &format!("{route} answered {status}"));
            return vec![e["error"].as_str().unwrap_or_default().to_owned()];
        }
        let lines = body
            .get("lines")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        lines.filter_map(Value::as_str).map(str::to_owned).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_strings_that_start_with_a_slash_are_mapped() {
        let v = json!({ "doc": { "source": "/real/a.wav", "name": "a" }, "n": 1, "list": ["/real/b", "c", "/other"] });
        let f = |s: &str| s.strip_prefix("/real").map(|r| format!("/w{r}"));
        let got = map_paths(&v, &f);
        let want = json!({ "doc": { "source": "/w/a.wav", "name": "a" }, "n": 1, "list": ["/w/b", "c", "/other"] });
        assert_eq!(got, want);
    }

    #[test]
    fn a_get_carries_its_args_as_the_query() {
        assert_eq!(
            query_of(&json!({ "clip": "a b", "whole": 1 })),
            "clip=a+b&whole=1"
        );
    }
}
