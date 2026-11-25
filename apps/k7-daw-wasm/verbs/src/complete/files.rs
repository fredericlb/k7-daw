// files.rs — the providers that read folders: library templates and chains, sequences, curves, playlists, audio, timeline pictures.
// A reader would get wrong the order: `Fs::list` is bytewise, so a name sorts as the TypeScript's `.sort()` sorts it, by code unit.

use super::Ctx;
use k7x::edit::Value;
use song::Kind;
use song_edit::Fs;
use song_edit::list_files::{ListOpts, list_files_under};
use song_edit::paths::join;
use song_edit::shared_folders::{Family, library_family_roots};
use song_edit::sugar::strip_doc_ext;
use song_edit::workspace_doc::ws_data;
use song_edit::workspace_file::is_doc_name;

const AUDIO_EXT: [&str; 8] = [".wav", ".aif", ".aiff", ".flac", ".mp3", ".ogg", ".m4a", ".opus"];

fn by_unit(a: &str, b: &str) -> std::cmp::Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// A folder's entries of `kind`, sorted; an unreadable folder has none.
fn names_in(fs: &dyn Fs, dir: &str, kind: Kind) -> Vec<String> {
    let mut out: Vec<String> = fs
        .list(dir)
        .map(|l| l.into_iter().filter(|e| e.kind == kind).map(|e| e.name).collect())
        .unwrap_or_default();
    out.sort_by(|a, b| by_unit(a, b));
    out
}

/// The document files of `dir` by id, the extension off.
fn doc_ids(fs: &dyn Fs, dir: &str) -> Vec<String> {
    names_in(fs, dir, Kind::File)
        .into_iter()
        .filter(|f| is_doc_name(f))
        .map(|f| strip_doc_ext(&f).to_owned())
        .collect()
}

/// `sequenceFiles`: the ids of `sequences/`, read off the folder.
pub fn sequence_files(ctx: &Ctx) -> Vec<String> {
    doc_ids(ctx.fs, &join(&[&ctx.song_path, "sequences"]))
}

/// `curves`: the ids of `curves/`, sorted.
pub fn curves(ctx: &Ctx) -> Vec<String> {
    doc_ids(ctx.fs, &join(&[&ctx.song_path, "curves"]))
}

/// `playlists`: the workspace's set lists, subfolders kept, the extension stripped.
pub fn playlists(ctx: &Ctx) -> Vec<String> {
    let opts = ListOpts {
        dotfiles: true,
        ..ListOpts::default()
    };
    let mut out: Vec<String> = list_files_under(ctx.fs, &join(&[&ctx.workspace(), "_playlists"]), &opts)
        .into_iter()
        .filter(|p| is_doc_name(p.rsplit('/').next().unwrap_or(p)))
        .map(|p| strip_doc_ext(&p).to_owned())
        .collect();
    out.sort_by(|a, b| by_unit(a, b));
    out
}

/// `audioFiles`: every audio file of the project, a hidden folder and `node_modules` skipped.
pub fn audio_files(ctx: &Ctx) -> Vec<String> {
    let opts = ListOpts {
        skip_dirs: &["node_modules"],
        ..ListOpts::default()
    };
    list_files_under(ctx.fs, &ctx.song_path, &opts)
        .into_iter()
        .filter(|p| {
            let lower = p.to_lowercase();
            AUDIO_EXT.iter().any(|e| lower.ends_with(e))
        })
        .collect()
}

/// `timelinePictures`: each timeline's picture lanes as (timeline, lane), off its file; an unreadable file has none.
pub fn timeline_pictures(ctx: &Ctx) -> Vec<(String, String)> {
    let dir = join(&[&ctx.song_path, "timelines"]);
    let mut out = Vec::new();
    for file in names_in(ctx.fs, &dir, Kind::File)
        .into_iter()
        .filter(|f| is_doc_name(f))
    {
        let Ok(data) = ws_data(ctx.fs, &join(&[&dir, &file])) else {
            continue;
        };
        if let Some(Value::List(pictures)) = data.get("pictures") {
            for p in pictures {
                if let Some(Value::Str(lane)) = p.get("lane") {
                    out.push((strip_doc_ext(&file).to_owned(), lane.clone()));
                }
            }
        }
    }
    out
}

/// `libraryAssemblies`: every assembly template the libraries hold, by ref.
pub fn library_assemblies(ctx: &Ctx) -> Vec<String> {
    let mut out = Vec::new();
    for lib in library_family_roots(ctx.fs, &ctx.env, Family::Assemblies, Some(&ctx.song_path)) {
        for name in names_in(ctx.fs, &lib.root, Kind::Dir) {
            walk_assemblies(
                ctx.fs,
                &join(&[&lib.root, &name]),
                &format!("@{}/{name}", lib.name),
                &mut out,
            );
        }
    }
    out
}

/// A folder holding an `assembly.k7x` IS a template, and nothing inside one is another.
fn walk_assemblies(fs: &dyn Fs, folder: &str, rf: &str, out: &mut Vec<String>) {
    if fs.exists(&join(&[folder, "assembly.k7x"])) {
        out.push(rf.to_owned());
        return;
    }
    for name in names_in(fs, folder, Kind::Dir) {
        walk_assemblies(fs, &join(&[folder, &name]), &format!("{rf}/{name}"), out);
    }
}

/// The refs of the chains the libraries hold: a file `<path>.k7x` under `chains/`, or a folder with a `graph.k7x`.
pub fn library_chains(ctx: &Ctx) -> Vec<String> {
    let mut out = Vec::new();
    for lib in library_family_roots(ctx.fs, &ctx.env, Family::Chains, Some(&ctx.song_path)) {
        walk_chains(ctx.fs, &lib.root, &format!("@{}", lib.name), &mut out);
    }
    out.sort_by(|a: &String, b: &String| a.chars().cmp(b.chars()));
    out.dedup();
    out
}

fn walk_chains(fs: &dyn Fs, at: &str, rf: &str, out: &mut Vec<String>) {
    let Ok(entries) = fs.list(at) else { return };
    if entries.iter().any(|e| e.kind == Kind::File && e.name == "graph.k7x") {
        // The library's own root is never a chain, only a folder below it.
        if rf.matches('/').count() > 0 {
            out.push(rf.to_owned());
        }
        return;
    }
    for e in entries {
        if e.kind == Kind::Dir {
            walk_chains(fs, &join(&[at, &e.name]), &format!("{rf}/{}", e.name), out);
        } else if is_doc_name(&e.name) {
            out.push(format!("{rf}/{}", strip_doc_ext(&e.name)));
        }
    }
}
