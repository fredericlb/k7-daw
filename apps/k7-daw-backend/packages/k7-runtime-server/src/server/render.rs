//! render.rs — `render`: a ProjectDoc bounced to wav INSIDE this engine, for a machine that may start no program,
//! answered with the manifest `--render` prints. One at a time, the machine's own clients only, every path held to
//! the roots before anything is read.
//! What a reader gets wrong: the bounce blocks, so it runs on a thread of its own, since it needs the message loop
//! running; the reply goes to the asker alone, stamped with the `id` the asker sent.

use super::ctx::{Ctx, Request};
use super::handlers::HandlerTable;
use super::msg::Malformed;
use crate::doc::{Checks, parse_project};
use crate::wire::json;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub fn add(t: &mut HandlerTable) {
    t.add(&["render"], render);
}

/// The doc with every relative clip source joined to `base`, so the roots test what the render will open.
fn with_base(mut doc: Value, base: &str) -> Value {
    if base.is_empty() {
        return doc;
    }
    let Some(Value::Array(tracks)) = doc.get_mut("tracks") else {
        return doc;
    };
    for clips in tracks
        .iter_mut()
        .filter_map(|t| t.get_mut("clips"))
        .filter_map(Value::as_array_mut)
    {
        for clip in clips {
            let Some(source) = clip.get("source").and_then(Value::as_str) else {
                continue;
            };
            let path = Path::new(source);
            if source.is_empty() || !path.is_relative() {
                continue;
            }
            let joined = normalize(&Path::new(base).join(path));
            clip["source"] = Value::String(joined.to_string_lossy().into_owned());
        }
    }
    doc
}

/// `lexically_normal`: `.` dropped and `..` folded against what precedes it, no file touched.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir
                if matches!(
                    out.components().next_back(),
                    Some(std::path::Component::Normal(_))
                ) =>
            {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn rendered(id: &Value, fields: Value) -> Value {
    let mut msg = json!({"type": "rendered", "id": id});
    if let (Value::Object(m), Value::Object(extra)) = (&mut msg, fields) {
        m.extend(extra);
    }
    msg
}

fn render(r: &Request) -> Result<(), Malformed> {
    let id = r.msg.get("id")?.cloned().unwrap_or(Value::Null);
    let refuse = |why: &str| {
        r.client
            .send(&rendered(&id, json!({"ok": false, "error": why})));
    };
    // It writes files and takes a core for seconds: a LAN device has no business asking.
    if !r.ctx.gate.locals().contains(&r.client.id) {
        refuse("only the machine running the engine can ask a render");
        return Ok(());
    }
    let out_dir = r.msg.str("outDir")?;
    let base = r.msg.str("base")?;
    let stems = r.msg.bool_or("stems", false)?;
    let doc = r.msg.get("doc")?.filter(|d| d.is_object()).cloned();
    let Some(doc) = doc.filter(|_| !out_dir.is_empty()) else {
        refuse("render: a doc and an outDir are required");
        return Ok(());
    };
    let engine = &r.ctx.engine;
    if !engine.path_allowed(&out_dir, true) {
        refuse(&format!(
            "render: '{out_dir}' is outside the folders this engine writes"
        ));
        return Ok(());
    }
    if !base.is_empty() && !engine.path_allowed(&base, false) {
        refuse(&format!(
            "render: '{base}' is outside the folders this engine reads"
        ));
        return Ok(());
    }
    // Refused WHOLE when one path is outside the roots: a render missing a clip is a wrong file, not a warning.
    let checks = Checks::of(engine);
    let parsed = match parse_project(&json!({"doc": with_base(doc, &base)}), "", &checks) {
        Ok(p) => p,
        Err(e) => {
            refuse(&format!("bad project: {}", e.0));
            return Ok(());
        }
    };
    if let Some(first) = checks.refused.borrow().first() {
        refuse(&format!(
            "render: '{first}' is outside the folders this engine may open"
        ));
        return Ok(());
    }
    if r.ctx.machine.rendering.swap(true, Ordering::SeqCst) {
        r.client.send(&rendered(
            &id,
            json!({"ok": false, "busy": true, "error": "a render is already running on this engine"}),
        ));
        return Ok(());
    }
    eprintln!("[render] started{}", if stems { " with stems" } else { "" });
    spawn(
        r.ctx.clone(),
        r.client.id,
        id,
        json::dump(&parsed.doc),
        out_dir,
        stems,
    );
    Ok(())
}

/// Clears the one-render flag on every way out of the worker.
struct Slot(Arc<Ctx>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.machine.rendering.store(false, Ordering::SeqCst);
    }
}

fn spawn(ctx: Arc<Ctx>, asker: u64, id: Value, doc: String, out_dir: String, stems: bool) {
    let worker = {
        let ctx = ctx.clone();
        move || {
            let _slot = Slot(ctx.clone());
            let outcome = ctx.engine.api().render_in_process(&doc, &out_dir, stems);
            if outcome.code != 0 {
                eprintln!("[render] {}", outcome.error);
                ctx.send_to(
                    asker,
                    &rendered(&id, json!({"ok": false, "error": outcome.error})),
                );
                return;
            }
            eprintln!("[render] done");
            let manifest = serde_json::from_str::<Value>(&outcome.manifest).unwrap_or(Value::Null);
            ctx.send_to(
                asker,
                &rendered(&id, json!({"ok": true, "manifest": manifest})),
            );
        }
    };
    if std::thread::Builder::new()
        .name("k7-render".into())
        .spawn(worker)
        .is_err()
    {
        ctx.machine.rendering.store(false, Ordering::SeqCst);
    }
}
