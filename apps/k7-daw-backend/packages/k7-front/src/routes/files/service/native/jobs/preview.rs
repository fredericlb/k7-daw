//! preview.rs — `Workspace.resolvePreview` over the front's parts: the crate's `resolve_preview` fed the account's home, its roots and its compile (2345).
//! What a reader gets wrong: it runs on a blocking thread, since the compile it may ask for (once, lazily) is a read of the disk.

use std::path::Path;
use std::sync::{Arc, OnceLock};

use k7_file_service::confine::project_roots;
use k7_file_service::jobs::preview::{
    ClipTable, DISK, DocSource, PreviewCtx, PreviewError, PreviewReq, Window, disk_defs,
    disk_exists, resolve_preview,
};
use k7_file_service::libraries::Home;
use k7_file_service::paths::join;
use serde_json::Value;
use song::PathRule;
use song::doc::ProjectDoc;

use crate::routes::files::service::native::engine::Host;

/// The active project's compiled document, made the first time an audition asks for it.
struct Compiled {
    host: Arc<dyn Host>,
    root: String,
    project: String,
    doc: OnceLock<Result<ProjectDoc, PreviewError>>,
}

impl DocSource for Compiled {
    fn table(&self) -> Result<&dyn ClipTable, PreviewError> {
        let made = self.doc.get_or_init(|| {
            let said = |message: String| PreviewError {
                message,
                code: None,
            };
            let doc = self.host.compile(&self.root, &self.project).map_err(said)?;
            serde_json::from_value::<ProjectDoc>(doc).map_err(|e| said(e.to_string()))
        });
        match made {
            Ok(doc) => Ok(doc),
            Err(e) => Err(e.clone()),
        }
    }
}

/// What an audition needs of the account beyond the request.
pub struct World<'a> {
    pub host: Arc<dyn Host>,
    pub home: &'a Home,
    pub code_root: &'a Path,
    pub models: Option<&'a str>,
    pub root: &'a str,
    pub active: Option<&'a str>,
}

/// The file and window the request plays, or the sentence of why not.
pub fn resolve(w: &World<'_>, req: &PreviewReq) -> Result<Window, PreviewError> {
    let meta = join(&[
        &w.code_root.display().to_string(),
        "apps",
        "k7-daw-frontend",
        "packages",
        "song",
        "meta",
    ]);
    let models = w
        .models
        .map_or_else(|| join(&[&w.home.studio, "models"]), str::to_owned);
    let roots = w
        .active
        .map(|p| project_roots(w.home, p, &meta, &models))
        .unwrap_or_default();
    let defs = disk_defs(&roots, PathRule::HOST);
    let doc = Compiled {
        host: w.host.clone(),
        root: w.root.to_owned(),
        project: w.active.unwrap_or_default().to_owned(),
        doc: OnceLock::new(),
    };
    let ctx = PreviewCtx {
        active: w.active,
        workspace_root: w.root,
        home: w.home,
        roots: &roots,
        probe: &DISK,
        doc: &doc,
        defs: &defs,
        exists: &disk_exists,
    };
    resolve_preview(&ctx, req)
}

/// The request's body as the audition asks it; the shape was checked by `check`.
pub fn request_of(body: &Value) -> PreviewReq {
    let text = |k: &str| body[k].as_str().unwrap_or_default().to_owned();
    let num = |k: &str| body[k].as_f64().unwrap_or_default();
    match body["kind"].as_str() {
        Some("clip") => PreviewReq::Clip { clip: text("clip") },
        Some("slice") => PreviewReq::Slice {
            source: text("source"),
            from: num("from"),
            to: num("to"),
        },
        _ => PreviewReq::Source {
            source: text("source"),
        },
    }
}
