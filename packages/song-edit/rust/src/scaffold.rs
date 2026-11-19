// scaffold.rs — a new project's files: the `arrangement` archetype and the `video` one, which adds a lane of picture over it.
// A reader would get wrong that a new project is written as chain files: no `graph.k7x`, no `nodes/`, and the ids are minted in a fixed order.

use crate::error::{Res, fail, io_error};
use crate::fs::Fs;
use crate::ids::NewId;
use crate::paths::{dirname, join, relative, resolve};
use crate::scaffold_files::{ARRANGEMENT, ARRANGEMENT_CHAINS, VIDEO_CHAINS, VIDEO_EXTRA};
use crate::shared_folders::SHARED_MODULES_PATH;
use crate::sugar::is_doc_file;
use crate::workspace_doc::ws_write_text;
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

/// Mint `count` ids and put them where a chain file says `<idN>`, N counting from 1.
pub(crate) fn with_ids(files: &[(&str, &str)], count: usize, new_id: NewId) -> Vec<(String, String)> {
    let ids: Vec<String> = (0..count).map(|_| new_id()).collect();
    files
        .iter()
        .map(|(k, t)| {
            let text = ids.iter().enumerate().fold((*t).to_owned(), |text, (i, id)| {
                text.replace(&format!("<id{}>", i + 1), id)
            });
            ((*k).to_owned(), text)
        })
        .collect()
}

fn statics(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files.iter().map(|(k, t)| ((*k).to_owned(), (*t).to_owned())).collect()
}

/// The `arrangement` archetype's files, chain files and all, keyed by project-relative path.
#[must_use]
pub fn project_files(name: &str, new_id: NewId) -> Vec<(String, String)> {
    let mut out = vec![("project.k7x".to_owned(), project_text(name, false))];
    out.extend(statics(ARRANGEMENT));
    out.extend(with_ids(ARRANGEMENT_CHAINS, 7, new_id));
    out
}

/// The `video` project's files: the arrangement ADDED to rather than restated.
#[must_use]
pub fn video_project_files(name: &str, new_id: NewId) -> Vec<(String, String)> {
    let mut out = vec![("project.k7x".to_owned(), project_text(name, true))];
    out.extend(statics(ARRANGEMENT));
    out.extend(statics(VIDEO_EXTRA));
    out.extend(with_ids(VIDEO_CHAINS, 9, new_id));
    out
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

/// Write a new project's files (the `video` shape copies `sample_video` in) and answer the files it holds.
///
/// # Errors
/// A missing sample, or what the writes fail with.
pub fn scaffold(
    fs: &mut dyn Fs,
    dir: &str,
    name: &str,
    shape: TimelineShape,
    sample_video: Option<&[u8]>,
    new_id: NewId,
) -> Res<Vec<String>> {
    let picture = shape == TimelineShape::Video;
    let files = if picture {
        video_project_files(name, new_id)
    } else {
        project_files(name, new_id)
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
    Ok(files_under(fs, dir))
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

/// The files a project holds, project-relative and in name order.
#[must_use]
pub fn files_under(fs: &dyn Fs, dir: &str) -> Vec<String> {
    let mut out = vec![];
    walk(fs, dir, dir, &mut out);
    out
}
