//! import_run.rs — `POST /import`: a foreign project (a file dropped on the home page, or one in the workspace) made a new project folder.
//! What a reader gets wrong: the samples are written first into the folder, then the texts land as one set; a set that fails takes the folder away.

use std::sync::Arc;

use axum::http::HeaderMap;
use axum::response::Response;
use k7_file_service::atomic::write_atomic;
use k7_file_service::changes::landing;
use k7_file_service::confine::Disk;
use k7_file_service::edits::import::{Plan, Refused, plan_dropped, plan_path};
use k7_file_service::edits::{posix, real};
use k7_file_service::space::Space;
use k7_file_service::workspaces::EXAMPLES;
use serde_json::{Value, json};

use super::entries::blocking;
use super::journal::{close_window, error, open_window};
use super::verb_host::plugins;
use super::{Ctx, landing as landing_route, locks};
use crate::routes::files::json_answer;
use crate::routes::files::service::routes::Call;

/// The most a dropped file's body may weigh: the file's own ceiling and a little for the framing.
pub const BODY_LIMIT: usize = k7_import::MAX_IMPORT_BYTES + (1 << 20);

/// The most a JSON request (a path in the workspace) may weigh.
pub const JSON_LIMIT: usize = 64 << 10;

/// The percent-decoded text of a header, none when it is absent or not UTF-8.
fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(name)?.to_str().ok()?;
    let mut out = Vec::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match bytes[i] {
            b'%' if i + 2 < bytes.len()
                && hex(bytes[i + 1]).is_some()
                && hex(bytes[i + 2]).is_some() =>
            {
                out.push((hex(bytes[i + 1])? * 16 + hex(bytes[i + 2])?) as u8);
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// A dropped file's request as the handler reads it: the file name and the folder asked for, from the headers.
pub fn dropped_meta(headers: &HeaderMap) -> Value {
    let mut meta = serde_json::Map::new();
    if let Some(f) = header(headers, "x-k7-file") {
        meta.insert("file".into(), json!(f));
    }
    if let Some(a) = header(headers, "x-k7-as") {
        meta.insert("as".into(), json!(a));
    }
    Value::Object(meta)
}

fn refused(r: &Refused) -> Response {
    let mut body = json!({ "error": r.message });
    if r.taken
        && let Some(o) = body.as_object_mut()
    {
        o.insert("taken".into(), json!(true));
    }
    json_answer(r.status, &body)
}

fn text<'a>(body: &'a Value, key: &str) -> Option<&'a str> {
    body.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// What writing the samples came to: the files written (to take away again), or why not.
fn write_binaries(ctx: &Ctx, plan: &Plan) -> Result<Vec<String>, Refused> {
    let fail = |status: u16, message: String| Refused {
        status,
        message,
        taken: false,
    };
    let space = Space::open(&ctx.home, &ctx.root);
    let folder = space
        .safe_path(&plan.project, &Disk)
        .map_err(|e| fail(403, e.to_string()))?;
    // The folder is claimed whole: a second import of the same name finds it and is told so.
    if let Err(e) = std::fs::create_dir(real(&posix(&folder))) {
        let taken = e.kind() == std::io::ErrorKind::AlreadyExists;
        return Err(Refused {
            status: if taken { 409 } else { 500 },
            message: if taken {
                format!("\"{}\" already exists", plan.project)
            } else {
                "the project's folder could not be made".into()
            },
            taken,
        });
    }
    let mut written = Vec::new();
    for (rel, bytes) in &plan.binaries {
        let inside = rel.starts_with(&format!("{}/", plan.project)) || rel.starts_with("_modules/");
        let abs = space
            .safe_path(rel, &Disk)
            .ok()
            .filter(|_| inside)
            .ok_or_else(|| fail(403, format!("'{rel}' is not a path an import writes")))?;
        let to = real(&posix(&abs));
        if let Some(dir) = std::path::Path::new(&to).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        write_atomic(&to, bytes).map_err(|_| fail(500, "a sample could not be written".into()))?;
        written.push(to);
    }
    Ok(written)
}

/// The shared files an import wrote, taken away again; the new folder is `landing`'s to remove.
fn unwrite(written: &[String]) {
    for file in written {
        let _ = std::fs::remove_file(file);
        if let Some(dir) = std::path::Path::new(file).parent() {
            let _ = std::fs::remove_dir(dir);
        }
    }
}

/// `POST /import`: `{rel, as?}` for a file of the workspace, or the file's bytes with `x-k7-file` and `x-k7-as?`; answers `{project, log}`.
pub async fn answer(c: &Call, ctx: &Ctx) -> Response {
    let typed = text(&c.body, "as").map(str::to_owned);
    let (rel, file) = (
        text(&c.body, "rel").map(str::to_owned),
        text(&c.body, "file").map(str::to_owned),
    );
    let raw = c.raw.clone();
    let source = match (&raw, &rel, &file) {
        (Some(_), _, Some(f)) => f.clone(),
        (None, Some(r), _) => r.clone(),
        (Some(_), _, None) => {
            return error(400, "a dropped file needs its name in the x-k7-file header");
        }
        (None, None, _) => return error(400, "an import needs a 'rel', or the file's bytes"),
    };
    if landing::is_examples(&ctx.root) {
        let why = format!(
            "{EXAMPLES} holds the bundled demos and takes no new project — make it in a workspace of your own"
        );
        return error(403, &why);
    }
    // The engine's plugin list, which names the plugins this machine cannot load; silence reads as unknown.
    let scan = match &c.engine {
        Some(e) => plugins::names(&e.link, &ctx.root, &ctx.account).await,
        None => None,
    };
    let (home, root, name) = (ctx.home.clone(), ctx.root.clone(), file.clone());
    let planned = blocking(move || {
        let space = Space::open(&home, &root);
        match (raw, rel) {
            (Some(bytes), _) => plan_dropped(
                space.root(),
                &name.unwrap_or_default(),
                &bytes,
                typed.as_deref(),
                scan,
            ),
            (None, Some(rel)) => plan_path(&space, &rel, typed.as_deref(), scan),
            (None, None) => Err(Refused {
                status: 400,
                message: "an import needs a 'rel', or the file's bytes".into(),
                taken: false,
            }),
        }
    })
    .await;
    let plan = match planned {
        Some(Ok(p)) => p,
        Some(Err(r)) => return refused(&r),
        None => return error(500, "internal error"),
    };
    if let Err(r) = landing::folder_name("", &plan.project) {
        return error(r.status(), &r.to_string());
    }
    let plan = Arc::new(plan);
    // The samples go first, into the folder the set then creates, in the capture's window as an upload's are.
    let window = open_window(c, &format!("import {source}")).await;
    let written = {
        let _lock = locks::write(&ctx.home.studio).await;
        let (ctx2, plan2) = (ctx.clone(), plan.clone());
        blocking(move || write_binaries(&ctx2, &plan2)).await
    };
    close_window(c, window).await;
    let written = match written {
        Some(Ok(w)) => w,
        Some(Err(r)) => return refused(&r),
        None => return error(500, "internal error"),
    };
    let ops: Vec<Value> = plan
        .texts
        .iter()
        .map(|(file, text)| json!({ "kind": "text", "file": file, "text": text, "was": null }))
        .collect();
    let mut set = c.clone();
    set.body = json!({
        "at": "",
        "base": "import",
        "label": format!("import {source}").chars().take(200).collect::<String>(),
        "create": plan.project,
        "ops": ops,
    });
    let answered = landing_route::answer(&set, ctx).await;
    if answered.status().as_u16() != 200 {
        unwrite(&written);
        return answered;
    }
    json_answer(200, &json!({ "project": plan.project, "log": plan.log }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_and_the_folder_ride_percent_encoded_in_headers() {
        let mut h = HeaderMap::new();
        h.insert(
            "x-k7-file",
            "m%C3%A9lodie%20%231.dawproject".parse().unwrap(),
        );
        h.insert("x-k7-as", "100%".parse().unwrap());
        let meta = dropped_meta(&h);
        assert_eq!(meta["file"], json!("m\u{e9}lodie #1.dawproject"));
        assert_eq!(meta["as"], json!("100%"));
        assert_eq!(dropped_meta(&HeaderMap::new()), json!({}));
    }
}
