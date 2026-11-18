// scaffold.rs — a new project's files: the `arrangement` archetype and the `video` one, which adds a lane of picture over it.
// A reader would get wrong that the files a scaffold answers are the CHAIN project's: the graph and `nodes/` are gone by then.

use crate::confine::Env;
use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::ids::NewId;
use crate::migrate_files::migrate_to_chain_files;
use crate::paths::{dirname, join, relative, resolve};
use crate::scaffold_files::{ARRANGEMENT, VIDEO_EXTRA};
use crate::shared_folders::SHARED_MODULES_PATH;
use crate::sugar::is_doc_file;
use crate::video_outputs::{add_render_sound, add_video_run, no_modules_map};
use crate::workspace_doc::ws_write_text;
use k7x::edit::Document;
use song::Kind;

pub const RENDER_AUDIO_LEVEL: u32 = 5;

/// The two archetypes with a TIMELINE.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineShape {
    Arrangement,
    Video,
}

/// The header field a lane of picture needs, written by the `video` archetype alone.
const PICTURE_HEADER: &str = concat!(
    "\n",
    "  // PICTURE. A song carries its own, and this field is what a lane of it needs.\n",
    "  //\n",
    "  // `viewport` is the frame every lane draws into: [width, height, fps]. There is no\n",
    "  // default — a frame invented by the loader would silently letterbox every shot, and you\n",
    "  // would find out at the export. The fps is used by EXPORT only; playing back follows\n",
    "  // the transport.\n",
    "  viewport: [1280, 720, 30],\n",
);

/// `JSON.stringify` of a string.
pub(crate) fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The project header; its picture half is the only field the two archetypes disagree about.
fn project_text(name: &str, picture: bool) -> String {
    format!(
        "project({{\n  name: {},\n  tempo: 120,\n  sampleRate: 48000,\n  timeSignature: \"4/4\",\n{}}})\n{}",
        quote(name),
        if picture { PICTURE_HEADER } else { "" },
        if picture {
            format!("format({RENDER_AUDIO_LEVEL})\n")
        } else {
            String::new()
        },
    )
}

/// The `arrangement` archetype's files, keyed by project-relative path, in the order they are written.
#[must_use]
pub fn project_files(name: &str) -> Vec<(String, String)> {
    let mut out = vec![("project.k7x".to_owned(), project_text(name, false))];
    out.extend(ARRANGEMENT.iter().map(|(k, t)| ((*k).to_owned(), (*t).to_owned())));
    out
}

/// The song's graph with the PICTURE's two outputs SPLICED in, by the open path's own edit.
fn video_graph(base: &str) -> Res<String> {
    let mut doc = Document::new(base);
    let mut header = doc.header();
    header.extend(
        [
            "",
            "…and the same pair for the PICTURE, which this project has: `RENDER` is where the video",
            "lanes composite — every one of them arrives there, as every audio lane arrives at `MIX` —",
            "and `VIDEO_OUT` is the frame that leaves. Put a video_effects module on the run between",
            "them and it works the whole composited frame. `RENDER#audio` is the sound the picture",
            "leaves with: the mix, here; unwire it and the export is silent. See packages/song/docs/videoOutputs.md.",
        ]
        .map(str::to_owned),
    );
    doc.set_header(&header)?;
    if !add_video_run(&mut doc.at(&[]))? || !add_render_sound(&mut doc.at(&[]))? {
        return no_modules_map();
    }
    Ok(doc.text().to_owned())
}

/// The `video` project's files: the arrangement ADDED to rather than restated.
///
/// # Errors
/// A base graph the edit refuses.
pub fn video_project_files(name: &str) -> Res<Vec<(String, String)>> {
    let mut out = project_files(name);
    for (k, t) in &mut out {
        match k.as_str() {
            "project.k7x" => *t = project_text(name, true),
            "graph.k7x" => *t = video_graph(t)?,
            _ => {}
        }
    }
    out.extend(VIDEO_EXTRA.iter().map(|(k, t)| ((*k).to_owned(), (*t).to_owned())));
    Ok(out)
}

/// Write a project's file map under `dir`, making the folders it implies; a `.k7x` must read, anything else goes as given.
///
/// # Errors
/// A `.k7x` that does not read, or the file system's.
pub fn write_project_files(fs: &mut dyn Fs, dir: &str, files: &[(String, String)]) -> Res<Vec<String>> {
    for (rel, content) in files {
        let path = join(&[dir, rel]);
        let folder = dirname(&path);
        fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
        if is_doc_file(rel) {
            ws_write_text(fs, &path, content)?;
        } else {
            fs.write_atomic(&path, content.as_bytes())
                .map_err(|e| io_error(&e, "open", &path))?;
        }
    }
    Ok(files.iter().map(|(k, _)| k.clone()).collect())
}

/// The 4OSC every scaffolded lane extends, written once per WORKSPACE and never over its own edit.
///
/// # Errors
/// The file system's.
pub fn ensure_workspace_synth(fs: &mut dyn Fs, dir: &str) -> Res<()> {
    let path = resolve(&format!("{dir}/{SHARED_MODULES_PATH}/instruments/synth/module.k7x"));
    if fs.exists(&path) {
        return Ok(());
    }
    let folder = dirname(&path);
    fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
    ws_write_text(
        fs,
        &path,
        concat!(
            "// \"synth\": Tracktion's built-in 4OSC. plugin.type is the FORMAT — \"internal\" for a\n",
            "// built-in, its xmlTypeName in `name` (an external plugin uses type \"vst3\" etc. and\n",
            "// its real name as `name`). Add params to tweak it.\n",
            "plugin({ type: \"internal\", name: \"4osc\" })\n",
        ),
    )?;
    Ok(())
}

/// Write a new project's files (the `video` shape copies `sample_video` in), then convert them to chain files.
///
/// # Errors
/// A missing sample, the conversion's failures, or what the writes fail with.
pub fn scaffold(
    fs: &mut dyn Fs,
    env: &Env,
    dir: &str,
    name: &str,
    shape: TimelineShape,
    sample_video: Option<&[u8]>,
    new_id: NewId,
) -> Res<Vec<String>> {
    let picture = shape == TimelineShape::Video;
    let files = if picture {
        video_project_files(name)?
    } else {
        project_files(name)
    };
    write_project_files(fs, dir, &files)?;
    ensure_workspace_synth(fs, dir)?;
    if picture {
        let Some(bytes) = sample_video else {
            return fail("bundled sample video missing — the @studio/song package is incomplete");
        };
        let folder = join(&[dir, "clips"]);
        fs.mkdir_all(&folder).map_err(|e| io_error(&e, "mkdir", &folder))?;
        let to = join(&[&folder, "sample.mp4"]);
        fs.write_atomic(&to, bytes).map_err(|e| io_error(&e, "copyfile", &to))?;
    }
    as_chain_project(fs, env, dir, new_id)
}

/// Every file under `at`, project-relative, folder by folder in name order.
fn walk(fs: &dyn Fs, root: &str, at: &str, out: &mut Vec<String>) {
    let mut entries = fs.list(at).unwrap_or_default();
    entries.sort_by_key(|e| e.name.encode_utf16().collect::<Vec<u16>>());
    for e in entries {
        let abs = join(&[at, &e.name]);
        if e.kind == Kind::Dir {
            walk(fs, root, &abs, out);
        } else {
            out.push(relative(root, &abs));
        }
    }
}

/// Convert a just-written project to chain files and answer the files it now holds.
///
/// # Errors
/// What the migration fails with.
pub fn as_chain_project(fs: &mut dyn Fs, env: &Env, dir: &str, new_id: NewId) -> Res<Vec<String>> {
    migrate_to_chain_files(fs, env, dir, new_id, false)?;
    let mut out = vec![];
    walk(fs, dir, dir, &mut out);
    Ok(out)
}
