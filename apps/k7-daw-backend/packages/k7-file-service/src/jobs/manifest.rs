//! manifest.rs — what a render's child or machine hands back: `parseManifest` of `renderer.ts`, and `moveInto` of `renderers.ts` (2320).
//! What a reader gets wrong: `move_into` departs on purpose (DEPARTS 2320b, pending the user): it copies only files inside
//! the staging folder, each name once (case folded), where the TypeScript copied any path a machine's reply named.

use crate::atomic::write_atomic_from;
use crate::confine::{Disk, confine_to};
use crate::paths::{basename, join};
use serde_json::Value;

/// JavaScript's `String.prototype.trim` set: the white-space characters minus U+0085, plus U+FEFF.
fn js_space(c: char) -> bool {
    (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}'
}

/// The last non-empty line of the child's stdout, which is a JSON value the TypeScript trusts as a manifest.
pub fn parse_manifest(stdout: &str) -> Result<Value, String> {
    let trimmed = stdout.trim_matches(js_space);
    let mut pieces: Vec<&str> = trimmed.split('\n').collect();
    let last = pieces.len().saturating_sub(1);
    // The separator is `\r?\n`: a `\r` before a newline goes with it, one at the very end stays.
    for p in pieces.iter_mut().take(last) {
        *p = p.strip_suffix('\r').unwrap_or(p);
    }
    let line = pieces.into_iter().rfind(|p| !p.is_empty()).unwrap_or("");
    serde_json::from_str(line)
        .map_err(|_| format!("could not read the engine's render manifest:\n{stdout}"))
}

/// A render's files as the caller's folder now holds them, by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Moved {
    pub mix: String,
    pub stems: Vec<(String, String)>,
}

/// The sentences a refusal says; `moveInto`'s own is the first.
pub const NO_MIXDOWN: &str = "the engine's machine rendered no mixdown";
pub const OUTSIDE: &str = "the engine's machine named a file outside its render";

fn node_copy_error(e: &std::io::Error, from: &str, to: &str) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => {
            format!("ENOENT: no such file or directory, copyfile '{from}' -> '{to}'")
        }
        _ => format!("{e}, copyfile '{from}' -> '{to}'"),
    }
}

/// Names a case-insensitive or Windows disk reads as one file: ASCII case folded, trailing dots and spaces dropped (DEPARTS 2320b).
fn same_file(a: &str, b: &str) -> bool {
    let fold = |n: &str| n.trim_end_matches(['.', ' ']).to_ascii_lowercase();
    fold(a) == fold(b)
}

/// The reply's files, uploaded under `staging`, copied into `out_dir` under their own names.
pub fn move_into(reply: &Value, staging: &str, out_dir: &str) -> Result<Moved, String> {
    let mix = reply.get("mix").and_then(Value::as_str).unwrap_or("");
    if mix.is_empty() {
        return Err(NO_MIXDOWN.to_owned());
    }
    // Stems with a string `name` and a string `file`; anything else is dropped, as the filter does.
    let stems: Vec<(&str, &str)> = reply
        .get("stems")
        .and_then(Value::as_array)
        .map(|l| {
            l.iter()
                .filter_map(|s| Some((s.get("name")?.as_str()?, s.get("file")?.as_str()?)))
                .collect()
        })
        .unwrap_or_default();
    // Every file is checked before any is copied: a refusal leaves the caller's folder as it was.
    let mut plan: Vec<(String, String)> = Vec::new();
    for file in std::iter::once(mix).chain(stems.iter().map(|(_, f)| *f)) {
        let name = basename(file);
        if name.is_empty() || name == "." || name == ".." {
            return Err(OUTSIDE.to_owned());
        }
        let Some(src) = confine_to(&[staging], file, &Disk) else {
            return Err(OUTSIDE.to_owned());
        };
        if plan.iter().any(|(n, _)| same_file(n, &name)) {
            return Err(format!("the engine's machine named two files '{name}'"));
        }
        plan.push((name, src));
    }
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    for (name, src) in &plan {
        let to = join(&[out_dir, name]);
        let copy = std::fs::File::open(src).and_then(|mut f| write_atomic_from(&to, &mut f));
        copy.map_err(|e| node_copy_error(&e, src, &to))?;
    }
    let name_of = |i: usize| plan[i].0.clone();
    Ok(Moved {
        mix: name_of(0),
        stems: stems
            .iter()
            .enumerate()
            .map(|(i, (n, _))| ((*n).to_owned(), name_of(i + 1)))
            .collect(),
    })
}
