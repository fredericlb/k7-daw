//! picture.rs — `picture`: the film of the scene `picture render` built, rendered in the background into the folder the person named.
//! What a reader gets wrong: the scene names its files by path, made `media/` URLs here and judged by the person's read gate.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::response::Response;
use k7_file_service::edits::picture::encode_component;
use k7_file_service::shares::picture::Quality;
use k7_file_service::verb_host::Error;
use k7_file_service::verb_host::jobs::Table;
use serde_json::{Map, Value, json};

use super::drive::{fail, no_machine};
use super::{Checked, failure, now, open_project};
use crate::routes::files::service::native::jobs::Jobs;
use crate::routes::files::service::native::runtime::Runtime;
use crate::routes::files::service::native::shares::host::{ports_of, script, settings};
use crate::routes::files::service::native::shares::picture::{
    BUSY, Browser, Request, media_by, render_with,
};
use crate::routes::files::service::native::shares::ports::RealViews;
use crate::routes::files::service::native::shares::{Account, registry};

/// The registry's tag, and how often a running render renews its lease.
const TAG: &str = "picture render";
const BEAT: Duration = Duration::from_secs(60);
/// No picture is wider or taller than this.
const MAX_SIDE: f64 = 16384.0;

fn bad(message: impl Into<String>) -> Response {
    failure(&Error::new(400, message))
}

/// A number field of `o` that is finite and at least `min`, or the 400 sentence.
fn number(o: &Map<String, Value>, key: &str, min: f64) -> Result<Option<f64>, String> {
    match o.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => match v.as_f64() {
            Some(n) if n.is_finite() && n >= min => Ok(Some(n)),
            _ => Err(format!("settings.{key} must be a number of at least {min}")),
        },
    }
}

/// The export's settings as `host::settings` reads them, each judged; `format` and `audio` ride beside them.
fn options(body: &Value) -> Result<Value, String> {
    let format = match body.get("format") {
        None | Some(Value::Null) => "webm",
        Some(Value::String(f)) if f == "webm" || f == "mp4" => f.as_str(),
        Some(_) => return Err("format must be webm or mp4".into()),
    };
    let audio = match body.get("audio") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err("audio must be a boolean".into()),
    };
    let empty = Map::new();
    let s = match body.get("settings") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(o)) => o,
        Some(_) => return Err("settings must be an object".into()),
    };
    let mut out = json!({ "format": format, "audio": audio });
    if let Some(v) = s.get("size").filter(|v| !v.is_null()) {
        let side = |k: &str| match v.get(k).and_then(Value::as_f64) {
            Some(n) if (1.0..=MAX_SIDE).contains(&n) => Ok(Some(n)),
            None if k == "width" => Ok(None),
            _ => Err(format!(
                "settings.size.{k} must be between 1 and {MAX_SIDE}"
            )),
        };
        out["size"] = json!({ "height": side("height")?, "width": side("width")? });
    }
    if let Some(q) = s.get("quality").filter(|v| !v.is_null()) {
        if q.as_str().and_then(Quality::parse).is_none() {
            return Err("settings.quality must be low, medium, high or max".into());
        }
        out["quality"] = q.clone();
    }
    let (from, to) = (number(s, "from", 0.0)?, number(s, "to", 0.0)?);
    if from.zip(to).is_some_and(|(f, t)| t <= f) {
        return Err("settings.to must come after settings.from".into());
    }
    for (k, v) in [("from", from), ("to", to)] {
        if let Some(n) = v {
            out[k] = json!(n);
        }
    }
    Ok(out)
}

/// The scene with each `src` and `lut` that names a path made the `media/` URL the render page fetches.
fn media_urls(v: &Value) -> Value {
    match v {
        Value::Object(o) => Value::Object(
            o.iter()
                .map(|(k, x)| match x {
                    Value::String(s) if (k == "src" || k == "lut") && s.starts_with('/') => {
                        (k.clone(), json!(format!("media/{}", encode_component(s))))
                    }
                    _ => (k.clone(), media_urls(x)),
                })
                .collect(),
        ),
        Value::Array(a) => Value::Array(a.iter().map(media_urls).collect()),
        _ => v.clone(),
    }
}

/// `POST /verb-host/picture {at, scene, into, format?, audio?, settings?}`: a job whose value is `{path}`, the film's file.
pub async fn picture(
    req: &Checked,
    jobs: &Arc<Jobs>,
    runtime: Option<Arc<Runtime>>,
    browser: Arc<dyn Browser>,
) -> Response {
    let Some(scene) = req.body.get("scene").filter(|s| s.is_object()) else {
        return bad("scene must be an object");
    };
    let Some(into) = req
        .body
        .get("into")
        .and_then(Value::as_str)
        .filter(|i| Path::new(i).is_absolute())
    else {
        return bad("into must be the absolute path of a folder");
    };
    // A folder inside a root reached the route as its virtual path: the film is written at its real one.
    let into = PathBuf::from(req.real(into).unwrap_or_else(|| into.to_owned()));
    let opts = match options(&req.body) {
        Ok(o) => o,
        Err(m) => return bad(m),
    };
    let at = req.body.get("at").and_then(Value::as_str);
    let active = match open_project(&req.ctx, at).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    let sound = opts["audio"] == json!(true);
    if sound && !jobs.renderer_available() {
        return no_machine("no engine machine to render the mixdown on");
    }
    let Some(docs) = req.ctx.docs.clone() else {
        return failure(&Error::coded(
            503,
            "the picture needs the native compiler",
            "off",
        ));
    };
    let shares = registry::of(&req.ctx.home.studio, &req.ctx.log);
    let busy = || failure(&Error::new(409, BUSY));
    if shares.picturing() {
        return busy();
    }
    // The scene's media are the files the person reads, judged one by one here.
    let scene = &media_urls(scene);
    let media = media_by(scene, &|rel| req.read(rel).ok().map(PathBuf::from));
    let tag = TAG.to_string();
    if runtime.as_ref().is_some_and(|r| !r.bg_start(&tag)) {
        return busy();
    }
    let run = match req.run() {
        Ok(r) => r,
        Err(r) => {
            if let Some(r) = &runtime {
                r.bg_end(&tag, None, Some("could not start"));
            }
            return *r;
        }
    };
    let id = run.id.clone();
    let env = docs.env.clone();
    let home = &req.ctx.home;
    let acct = Account {
        root: PathBuf::from(&req.ctx.root),
        tmp: ["TMPDIR", "TEMP", "TMP"]
            .iter()
            .find_map(|k| env.get(*k).filter(|v| !v.is_empty()))
            .map_or_else(std::env::temp_dir, PathBuf::from),
        env,
    };
    let views = RealViews {
        docs: docs.clone().with_thawed(jobs.thawed.clone()),
        home: home.clone(),
        log: req.ctx.log.clone(),
    };
    let ports = ports_of(jobs.clone(), Arc::new(views), browser);
    let (scene_text, project, job) = (scene.to_string(), PathBuf::from(&active), id.clone());
    let log = req.ctx.log.clone();
    tokio::spawn(async move {
        let settings = settings(&opts);
        let page = script(&|k| acct.var(k), Path::new(&docs.code_root));
        let none = |_: &str| -> Option<PathBuf> { None };
        let request = Request {
            project: &project,
            into: &into,
            scene: &scene_text,
            sound: if sound { "mix" } else { "" },
            opts: &settings,
            script: page,
            debug: acct
                .var("STUDIO_RENDER_DEBUG")
                .is_some_and(|v| !v.is_empty()),
            resolve: &none,
        };
        let mut work = Box::pin(render_with(&shares, &acct, &ports, request, Some(media)));
        let result = loop {
            tokio::select! {
                r = &mut work => break r,
                () = tokio::time::sleep(BEAT) => {
                    if let Some(r) = &runtime {
                        r.bg_progress(&tag, None, None);
                    }
                }
            }
        };
        drop(work);
        match &result {
            Ok(path) => {
                Table::global().finish(&job, json!({ "path": path }), now());
                log(&format!("picture rendered: {path}"));
            }
            Err(e) => fail(&job, e, None),
        }
        if let Some(r) = &runtime {
            match &result {
                Ok(path) => r.bg_end(&tag, Some(&format!("picture rendered: {path}")), None),
                Err(e) => r.bg_end(&tag, None, Some(e)),
            }
        }
        drop(run);
    });
    Checked::accepted(&id)
}

#[cfg(test)]
mod tests {
    use super::media_urls;
    use serde_json::json;

    #[test]
    fn a_scene_path_becomes_the_media_url_the_render_page_fetches() {
        let scene = json!({ "layers": [{ "clips": [{ "src": "/w/a b.mp4" }, { "src": "" }], "passes": [{ "lut": "/w/l.cube", "fragment": "/* x */" }] }] });
        let got = media_urls(&scene);
        assert_eq!(
            got["layers"][0]["clips"][0]["src"],
            "media/%2Fw%2Fa%20b.mp4"
        );
        assert_eq!(got["layers"][0]["clips"][1]["src"], "");
        assert_eq!(got["layers"][0]["passes"][0]["lut"], "media/%2Fw%2Fl.cube");
        assert_eq!(got["layers"][0]["passes"][0]["fragment"], "/* x */");
    }
}
