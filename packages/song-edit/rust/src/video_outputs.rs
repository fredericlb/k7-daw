// video_outputs.rs — the picture's two built-in outputs as graph edits: the `RENDER` run and the mix wired into its sound input.
// A reader would get wrong that the open-time `ensure*` calls are best-effort: a failure writes nothing and says false.

use crate::chain_create::chain_taken;
use crate::chain_data::Start;
use crate::chain_graph_edit::edit_project_graph;
use crate::chain_item_edit::{CHAINS_DIR, is_chain_project};
use crate::chain_project::read_chain_project;
use crate::confine::Env;
use crate::error::Res;
use crate::format_level::RENDER_AUDIO_LEVEL;
use crate::format_stamp::{declared_level, stamp_format_level};
use crate::fs::Fs;
use crate::graph_file::connect_nodes;
use crate::graph_schema::GRAPH_FILE;
use crate::module_io::entity_file_for;
use crate::paths::join;
use crate::workspace_doc::{ws_data, ws_text, ws_write_text};
use k7x::edit::{Document, PathSeg, Scope, StringifyOptions, Tree, Value, WriteOptions, stringify};

pub const RENDER_NODE: &str = "RENDER";
pub const VIDEO_OUT_NODE: &str = "VIDEO_OUT";
pub const MIX_NODE: &str = "MIX";
pub const RENDER_AUDIO_IN: &str = "RENDER#audio";

fn key(s: &str) -> PathSeg {
    PathSeg::Key(s.to_owned())
}

/// Whether `modules` is absent or a map: the only shapes a run can be added to.
fn modules_ok(doc: &Scope) -> Res<bool> {
    Ok(doc.node_at(&[key("modules")], |n| n.is_none_or(|t| matches!(t, Tree::Map(_))))?)
}

/// Append the run to the END of the `modules` map, made when the file has none; false when `modules` is not a map.
///
/// # Errors
/// A document the editor refuses.
pub fn add_video_run(doc: &mut Scope) -> Res<bool> {
    if !modules_ok(doc)? {
        return Ok(false);
    }
    doc.set_in(
        &[key("modules"), key(RENDER_NODE)],
        &Value::List(vec![Value::Str(VIDEO_OUT_NODE.to_owned())]),
        WriteOptions::default(),
    )?;
    Ok(true)
}

/// Wire the mix into RENDER's sound input unless something already is; false when `modules` is not a map.
///
/// # Errors
/// A graph that does not read, or an edit it refuses.
pub fn add_render_sound(doc: &mut Scope) -> Res<bool> {
    if !modules_ok(doc)? {
        return Ok(false);
    }
    match connect_nodes(doc, MIX_NODE, RENDER_AUDIO_IN) {
        // Wired already, by hand or by a `runs` line: the arrow the file holds stays as it is.
        Err(e) if e.to_string().contains("already feeds") => {}
        Err(e) => return Err(e),
        Ok(()) => {}
    }
    Ok(true)
}

/// Wire the mix into RENDER#audio ONCE, the first time a build of level 5 opens it, then stamp it.
pub fn ensure_render_sound(fs: &mut dyn Fs, env: &Env, dir: &str) -> bool {
    let Some(root) = entity_file_for(fs, dir, "", "project") else {
        return false;
    };
    let Ok(level) = declared_level(fs, &root.path) else {
        return false;
    };
    if level >= RENDER_AUDIO_LEVEL {
        return false;
    }
    // A project with no graph file fails here, and is left unstamped like any failed open.
    if edit_project_graph(fs, env, dir, |g| add_render_sound(g).map(|_| ())).is_err() {
        return false;
    }
    stamp_format_level(fs, dir, RENDER_AUDIO_LEVEL)
}

/// Give `dir`'s graph the run if it has none, and its sound once: the CALLER decides there is a viewport.
pub fn ensure_video_run(fs: &mut dyn Fs, env: &Env, dir: &str) -> bool {
    let wrote = ensure_run(fs, dir);
    // After the run, so a project gaining both gets its sound wired into the RENDER just written.
    ensure_render_sound(fs, env, dir);
    wrote
}

fn ensure_run(fs: &mut dyn Fs, dir: &str) -> bool {
    if is_chain_project(fs, dir) {
        return ensure_chain_video_run(fs, dir);
    }
    let file = join(&[dir, GRAPH_FILE]);
    if !fs.exists(&file) {
        return false;
    }
    let run = |fs: &mut dyn Fs| -> Res<bool> {
        let has = matches!(ws_data(fs, &file)?.get("modules"), Some(Value::Map(m)) if m.iter().any(|(k, _)| k == RENDER_NODE));
        if has {
            return Ok(false);
        }
        // The file as written, not its expansion: a `runs` short form stays, since the run joins `modules` beside it.
        let mut doc = Document::new(&ws_text(fs, &file)?);
        if !add_video_run(&mut doc.at(&[]))? {
            return Ok(false);
        }
        ws_write_text(fs, &file, doc.text())?;
        Ok(true)
    };
    run(fs).unwrap_or(false)
}

/// The same in a chain project: a chain starting at RENDER, written when none does.
fn ensure_chain_video_run(fs: &mut dyn Fs, dir: &str) -> bool {
    let Ok(cp) = read_chain_project(fs, dir) else {
        return false;
    };
    if cp.chains.values().any(|c| c.start == Some(Start::Render)) {
        return false;
    }
    // A chain file that would not read may be the one that starts there: guessing would write a second.
    if !cp.issues.is_empty() {
        return false;
    }
    let mut name = "picture".to_owned();
    let mut i = 2;
    while chain_taken(fs, dir, &name) {
        name = format!("picture-{i}");
        i += 1;
    }
    let step = Value::Map(vec![(
        "to".to_owned(),
        Value::List(vec![Value::Map(vec![(
            "chain".to_owned(),
            Value::Str(VIDEO_OUT_NODE.to_owned()),
        )])]),
    )]);
    let chain = Value::Map(vec![
        ("start".to_owned(), Value::Str(RENDER_NODE.to_owned())),
        ("steps".to_owned(), Value::List(vec![step])),
    ]);
    let Ok(text) = stringify(
        &chain,
        &StringifyOptions {
            header: &[],
            inline: None,
        },
    ) else {
        return false;
    };
    ws_write_text(fs, &join(&[dir, CHAINS_DIR, &format!("{name}.k7x")]), &text).is_ok()
}
