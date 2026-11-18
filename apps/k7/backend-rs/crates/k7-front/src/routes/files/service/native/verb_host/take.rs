//! take.rs — `take/save`: a take and its picture checked against the record directory, by the media piece's own `save_take`.
//! What a reader gets wrong: the record directory comes from the Worker's compile, so it is read through the gate like any path.

use std::path::PathBuf;

use axum::response::Response;
use k7_file_service::verb_host::Error;
use serde_json::json;

use super::{Checked, failure};
use crate::routes::files::json_answer;
use crate::routes::files::service::native::media::Counts;
use crate::routes::files::service::native::media::take::{self, Cfg, LANDING, LOOK, Request};

fn optional(req: &Checked, key: &str) -> Result<Option<String>, Box<Response>> {
    match req.body.get(key) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) if s.is_empty() => Ok(None),
        Some(serde_json::Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(Box::new(failure(&Error::new(
            400,
            format!("{key} must be a string"),
        )))),
    }
}

/// `POST /verb-host/take/save {at, recordDir, file?, video?, label?}`: the first file's name in the record directory.
pub async fn save(req: &Checked) -> Response {
    let shown = match req.text("recordDir") {
        Ok(s) => s.to_string(),
        Err(r) => return *r,
    };
    let record_dir = match req.read(&shown) {
        Ok(d) => d,
        Err(r) => return *r,
    };
    let (file, video) = match (
        optional(req, "file"),
        optional(req, "video"),
        optional(req, "label").map(|_| ()),
    ) {
        (Ok(f), Ok(v), Ok(())) => (f, v),
        (Err(r), _, _) | (_, Err(r), _) | (_, _, Err(r)) => return *r,
    };
    // A take the engine wrote is named by its real path or its name; one the page named under a root goes through the gate.
    let file = match file {
        Some(f) if req.virt.real(&f).is_some() => match req.read(&f) {
            Ok(r) => Some(r),
            Err(r) => return *r,
        },
        other => other,
    };
    let video = match video {
        Some(v) => match req.read(&v) {
            Ok(r) => Some(r),
            Err(r) => return *r,
        },
        None => None,
    };
    let cfg = Cfg {
        roots: [&req.ctx.home.studio, &req.ctx.home.libraries]
            .iter()
            .map(PathBuf::from)
            .collect(),
        landing: LANDING,
        look: LOOK,
    };
    let ask = Request {
        record_dir,
        file,
        video,
    };
    match take::save_take(&cfg, &Counts::default(), &ask).await {
        Ok(saved) => json_answer(200, &json!({ "path": saved.path })),
        Err(m) if m == "nothing was recorded" => failure(&Error::new(400, m)),
        Err(m) => failure(&Error::new(500, m)),
    }
}
