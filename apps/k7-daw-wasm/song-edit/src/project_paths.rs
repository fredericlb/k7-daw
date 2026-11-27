// project_paths.rs — which KIND of project file a path is, and the project-relative forms of it: the page's one copy of these rules.
// A reader would get wrong the dot of `.k7x`: the page's patterns leave it unescaped (sugar.rs), so `x_k7x` ends a document name here too.

use crate::sugar::strip_doc_ext;
use k7x::edit::is_js_space;
use k7x::scan::scalar_field;
use std::collections::HashMap;

/// A path with forward slashes, whatever the platform handed us.
#[must_use]
pub fn norm(path: &str) -> String {
    path.replace('\\', "/")
}

/// What precedes the document extension (`.k7x`, the dot any character but a line end), or None for a name without it.
fn doc_head(p: &str) -> Option<&str> {
    let b = p.as_bytes();
    let n = b.len();
    if n < 3 || !b[n - 3..].eq_ignore_ascii_case(b"k7x") {
        return None;
    }
    let rest = &p[..n - 3];
    let c = rest.chars().next_back()?;
    (!matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')).then(|| &rest[..rest.len() - c.len_utf8()])
}

/// `s` without a trailing `tail`, compared without ASCII case.
fn strip_tail_ci<'a>(s: &'a str, tail: &str) -> Option<&'a str> {
    let (b, t) = (s.as_bytes(), tail.as_bytes());
    (b.len() >= t.len() && b[b.len() - t.len()..].eq_ignore_ascii_case(t)).then(|| &s[..s.len() - t.len()])
}

/// Does `s` end in one of `names` as a whole segment (the whole string, or after a `/`)?
fn ends_segment(s: &str, names: &[&str]) -> bool {
    names
        .iter()
        .any(|name| strip_tail_ci(s, name).is_some_and(|before| before.is_empty() || before.ends_with('/')))
}

/// The file name of `(^|/)<folder>/[^/]+<doc>$`, or None.
fn in_folder<'a>(p: &'a str, folder: &str) -> Option<&'a str> {
    let (before, name) = doc_head(p)?.rsplit_once('/')?;
    (!name.is_empty() && ends_segment(before, &[folder])).then_some(name)
}

/// A MIDI sequence file: the editor previews its inline notes.
#[must_use]
pub fn is_seq_file(path: &str) -> bool {
    in_folder(&norm(path), "sequences").is_some()
}

/// A clip file: audio (a waveform) or video.
#[must_use]
pub fn is_clip_file(path: &str) -> bool {
    in_folder(&norm(path), "clips").is_some()
}

/// A LANE's file, a node in `nodes/` or a chain project's `tracks/`; deliberately loose.
#[must_use]
pub fn is_track_file(path: &str) -> bool {
    let p = norm(path);
    in_folder(&p, "nodes").is_some() || in_folder(&p, "tracks").is_some()
}

/// A PICTURE lane's file: a lane file named after one of `picture_lanes`.
#[must_use]
pub fn is_video_track_file(path: &str, picture_lanes: &[String]) -> bool {
    is_track_file(path) && picture_lanes.contains(&file_stem(path))
}

/// A module folder's own `module.k7x`, tested as the page's pattern does: no slash folding, so `a\\module.k7x` is no module.
#[must_use]
pub fn is_own_module_file(path: &str) -> bool {
    doc_head(path).is_some_and(|head| ends_segment(head, &["module"]))
}

/// The two segments between `modules/` and `module.k7x`, or None: `(^|/)modules/[^/]+/[^/]+/module<doc>$`.
fn rack_module_parts(p: &str) -> Option<(&str, &str)> {
    let before = strip_tail_ci(doc_head(p)?, "/module")?;
    let (rest, last) = before.rsplit_once('/')?;
    let (head, first) = rest.rsplit_once('/')?;
    (!last.is_empty() && !first.is_empty() && ends_segment(head, &["modules"])).then_some((first, last))
}

/// A module file whose rack's `source:` names a file BESIDE it, rather than the project-relative path a clip's source is.
#[must_use]
pub fn is_rack_module_file(path: &str) -> bool {
    rack_module_parts(&norm(path)).is_some()
}

/// The chain-`use` key a file answers to in `hints.racks`: a module's folder name or a NODE's stem.
#[must_use]
pub fn rack_key_of(path: &str) -> Option<String> {
    let p = norm(path);
    if let Some((_, last)) = rack_module_parts(&p) {
        return Some(last.to_owned());
    }
    in_folder(&p, "nodes").map(str::to_owned)
}

/// A module file the BUILDER opens: both trees, since the builder edits both and its routes take either.
#[must_use]
pub fn is_module_file(path: &str) -> bool {
    let p = norm(path);
    let Some(head) = doc_head(&p) else { return false };
    let Some(before) = strip_tail_ci(head, "/module").or_else(|| strip_tail_ci(head, "/ui")) else {
        return false;
    };
    let roots = ["modules", "_modules"];
    let Some((a, x)) = before.rsplit_once('/') else {
        return false;
    };
    if !x.is_empty() && ends_segment(a, &roots) {
        return true;
    }
    let Some((b, first)) = a.rsplit_once('/') else {
        return false;
    };
    !x.is_empty() && !first.is_empty() && ends_segment(b, &roots)
}

/// JavaScript's `Intl`-free case fold for one code unit: what a regular expression's `i` flag compares.
fn fold(c: char) -> char {
    if c as u32 > 0xFFFF {
        return c;
    }
    let mut up = c.to_uppercase();
    match (up.next(), up.next()) {
        (Some(u), None) if !(c as u32 >= 128 && (u as u32) < 128) => u,
        _ => c,
    }
}

/// The first file of `files` that IS `<folder>/<id>`, or None: a lookup and not a search, since `tracks/kick` and `clips/kick` both exist.
#[must_use]
pub fn entity_file(files: &[String], folder: &str, id: &str) -> Option<String> {
    let needle: Vec<char> = folder
        .chars()
        .chain(std::iter::once('/'))
        .chain(id.chars())
        .map(fold)
        .collect();
    files
        .iter()
        .find(|f| {
            let p = norm(f);
            let Some(head) = doc_head(&p) else { return false };
            let tail: Vec<char> = head.chars().rev().take(needle.len()).collect();
            if tail.len() < needle.len() || !tail.iter().rev().zip(&needle).all(|(a, b)| fold(*a) == *b) {
                return false;
            }
            let cut: usize = tail.iter().map(|c| c.len_utf8()).sum();
            let before = &head[..head.len() - cut];
            before.is_empty() || before.ends_with('/')
        })
        .cloned()
}

/// A file's id: its name without the folder or the extension.
#[must_use]
pub fn file_stem(path: &str) -> String {
    let p = norm(path);
    strip_doc_ext(p.rsplit('/').next().unwrap_or("")).to_owned()
}

/// A project folder, slashed and without a trailing `/`; "" when there is none or it is the workspace itself (`.`).
#[must_use]
pub fn project_dir(active_project: Option<&str>) -> String {
    let p = norm(active_project.unwrap_or(""));
    let ap = p.trim_end_matches('/');
    if ap == "." { String::new() } else { ap.to_owned() }
}

fn inside(p: &str, ap: &str) -> bool {
    p == ap || p.strip_prefix(ap).is_some_and(|rest| rest.starts_with('/'))
}

/// A workspace-relative path made relative to the project folder, or merely slashed when it lies outside it.
#[must_use]
pub fn to_project_relative(path: &str, active_project: Option<&str>) -> String {
    let p = norm(path);
    let ap = project_dir(active_project);
    if !ap.is_empty() && inside(&p, &ap) {
        return p.get(ap.len() + 1..).unwrap_or("").to_owned();
    }
    p
}

/// Is this path inside the project the ENGINE holds? The tree is the whole WORKSPACE, so a clip may belong to a project that is not playing.
#[must_use]
pub fn in_active_project(path: &str, active_project: Option<&str>) -> bool {
    let Some(_) = active_project else { return false };
    let ap = project_dir(active_project);
    ap.is_empty() || inside(&norm(path), &ap)
}

/// The generic module file beside `rel`: a node's sibling.
#[must_use]
pub fn module_file_of(rel: &str) -> String {
    let n = norm(rel);
    if let Some(head) = doc_head(&n)
        && let Some((prefix, name)) = head.rsplit_once('/')
        && !name.is_empty()
    {
        return format!("{prefix}/module.k7x");
    }
    n
}

/// The folder a path sits in: "" at the root, which every file route reads as the workspace.
#[must_use]
pub fn parent_dir(path: &str) -> String {
    let p = norm(path);
    let p = p.trim_end_matches('/');
    p.rfind('/').map_or_else(String::new, |i| p[..i].to_owned())
}

/// The last segment of a path: the name a rename starts from.
#[must_use]
pub fn base_name(path: &str) -> String {
    let p = norm(path);
    let p = p.trim_end_matches('/');
    p.rsplit('/').next().map_or_else(|| path.to_owned(), str::to_owned)
}

/// A line split into its leading `n` tokens and the rest, so a name with spaces survives while the path before it stays one token.
#[must_use]
pub fn head_tokens(rest: &str, n: usize) -> Vec<String> {
    let trim = |s: &str| s.trim_matches(is_js_space).to_owned();
    let mut out = Vec::new();
    let mut s = trim(rest);
    while out.len() + 1 < n {
        let Some(i) = s.find(is_js_space) else { break };
        out.push(s[..i].to_owned());
        s = trim(&s[i..]);
    }
    if !s.is_empty() {
        out.push(s);
    }
    out
}

/// Resolve a gutter source where the COMPILER puts it: a rack's sits beside its file, a clip's at the project root.
#[must_use]
pub fn resolve_gutter_source(source: &str, file_path: &str, active_project: Option<&str>) -> String {
    let mut chars = source.chars();
    let rooted = match (chars.next(), chars.next(), chars.next()) {
        (Some('\\' | '/'), ..) => true,
        (Some(d), Some(':'), Some('\\' | '/')) => d.is_ascii_alphabetic(),
        _ => false,
    };
    if source.is_empty() || rooted {
        return source.to_owned();
    }
    let rel = to_project_relative(file_path, active_project);
    if !is_rack_module_file(&rel) {
        return source.to_owned();
    }
    match rel.rfind('/') {
        Some(slash) if slash > 0 => format!("{}/{source}", &rel[..slash]),
        _ => source.to_owned(),
    }
}

/// The module FILE behind an instance's `extends:`: `base:` first, then `module:` for a local ref; None when it is the file itself.
#[must_use]
pub fn module_behind(
    content: &str,
    paths: &HashMap<String, String>,
    project_root: &str,
    own_path: Option<&str>,
) -> Option<String> {
    let reference = scalar_field(content, "extends")?;
    let rel = paths
        .get(&format!("base:{reference}"))
        .or_else(|| paths.get(&format!("module:{reference}")))?;
    if rel.is_empty() {
        return None;
    }
    let abs: Vec<&str> = std::iter::once(project_root)
        .chain(rel.split('/'))
        .filter(|s| !s.is_empty())
        .collect();
    let abs = abs.join("/");
    // The instance and its module can be one file, and the same buffer twice is not a comparison but a mirror.
    (own_path != Some(abs.as_str()) && own_path != Some(rel.as_str())).then_some(abs)
}

/// The kind of a clip file's source: audio unless it ends in a picture extension. Reads the text with a scan, so a half-typed file answers.
#[must_use]
pub fn is_audio_clip_file(text: &str) -> bool {
    scalar_field(text, "source").is_some_and(|s| !is_video_source(&s))
}

/// A source ending in a picture container: `\.(mp4|mov|m4v|webm|mkv)$` without case.
#[must_use]
pub fn is_video_source(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    song::wire::VIDEO_EXTS
        .iter()
        .any(|e| lower.strip_suffix(e).is_some_and(|h| h.ends_with('.')))
}

/// The rack module a sequence names via `use_rack`, so a slice rack can dock its source waveform.
#[must_use]
pub fn find_use_rack(text: &str) -> Option<String> {
    scalar_field(text, "use_rack")
}
