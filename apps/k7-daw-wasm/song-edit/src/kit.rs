// kit.rs — the rack builder's rules: which pad fires which note, where the samples go, what stops a CREATE, and the KIT node it commits.
// A reader would get wrong that the node is a node of the library's `k7-rack` on its `samples` engine, carrying its own taps (song/docs/racks.md).

use crate::project_paths::project_dir;
use crate::rack_taps::RACK_FIRST_NOTE;
use k7x::edit::{EditError, PathSeg, StringifyOptions, Value, stringify};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The grid's side: 4×4, sixteen pads, the drum-machine page.
pub const KIT_GRID: usize = 4;

/// The rack every kit is a node of: a LIBRARY module whose engines are its kits, the kit being the node's.
const RACK_MODULE: &str = "instruments/k7-rack";
/// …and its SAMPLER engine, the one that plays the node's own taps; the module's default is an SFZ kit.
const RACK_ENGINE: &str = "samples";

/// The MIDI note a pad fires: the grid READS like a drum machine, bottom-left 36 to top-right 51, with `row` counting from the TOP.
#[must_use]
pub fn pad_note(row: usize, col: usize) -> f64 {
    RACK_FIRST_NOTE + ((KIT_GRID - 1 - row) * KIT_GRID + col) as f64
}

/// Every cell in NOTE order, the order the symbols are WRITTEN in, so the hints and the colours read the grid as it plays.
#[must_use]
pub fn cells_in_note_order() -> Vec<(usize, usize)> {
    (0..KIT_GRID)
        .rev()
        .flat_map(|row| (0..KIT_GRID).map(move |col| (row, col)))
        .collect()
}

/// A pad's default symbol: the file's stem, upper-cased and squeezed to the characters a symbol is addressed by. Empty in, `PAD` out.
#[must_use]
pub fn default_symbol(file_name: &str) -> String {
    // `\.[^.]*$`: from the last dot on.
    let stem = file_name.rfind('.').map_or(file_name, |i| &file_name[..i]);
    let mut out = String::new();
    let mut run = false;
    for c in stem.to_uppercase().chars() {
        if c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_' || c == '-' {
            out.push(c);
            run = false;
        } else if !run {
            out.push('_');
            run = true;
        }
    }
    let clean = out.trim_matches('_');
    if clean.is_empty() {
        "PAD".to_owned()
    } else {
        clean.to_owned()
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct RackPad {
    pub symbol: String,
    /// As it will sit in the kit's folder under `samples/`.
    pub file_name: String,
    pub note: f64,
}

/// Which library's sampler a kit extends: `@k7-core`'s first, else any library's; None when none holds one.
#[must_use]
pub fn rack_module_ref(base_modules: &[String]) -> Option<String> {
    let suffix = format!("/{RACK_MODULE}");
    let found: Vec<&String> = base_modules
        .iter()
        .filter(|r| {
            r.strip_prefix('@')
                .and_then(|rest| rest.strip_suffix(&suffix))
                .is_some_and(|lib| !lib.is_empty() && !lib.contains('/') && !matches!(lib, "workspace" | "engine"))
        })
        .collect();
    found
        .iter()
        .find(|r| r.starts_with("@k7-core/"))
        .or(found.first())
        .map(|r| (*r).clone())
}

/// Where a kit's samples land: the PROJECT's own `samples/`, or one of the account's libraries'.
/// The one word that says the project itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ProjectWord {
    Project,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(untagged)]
pub enum KitDestination {
    Project(ProjectWord),
    Library { library: String },
}

/// The folder the samples are uploaded into, workspace-relative; `Project` is the active project's folder.
#[must_use]
pub fn kit_upload_dir(dest: &KitDestination, kit: &str, project: &str) -> String {
    match dest {
        KitDestination::Project(_) => {
            let p = project_dir(Some(project));
            format!(
                "{}samples/{kit}",
                if p.is_empty() { String::new() } else { format!("{p}/") }
            )
        }
        KitDestination::Library { library: lib } => format!("_libraries/{lib}/samples/{kit}"),
    }
}

/// The name a tap writes for one of them: `@project/<kit>/<file>` or `@<library>/<kit>/<file>`.
#[must_use]
pub fn kit_sample_ref(dest: &KitDestination, kit: &str, file: &str) -> String {
    let owner = match dest {
        KitDestination::Project(_) => "project",
        KitDestination::Library { library: l } => l,
    };
    format!("@{owner}/{kit}/{file}")
}

/// What stops a CREATE: a message key and the parameters its sentence takes.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RackIssue {
    pub key: &'static str,
    pub params: BTreeMap<&'static str, String>,
}

fn issue(key: &'static str) -> RackIssue {
    RackIssue {
        key,
        params: BTreeMap::new(),
    }
}

fn with(key: &'static str, name: &'static str, value: &str) -> RackIssue {
    RackIssue {
        key,
        params: BTreeMap::from([(name, value.to_owned())]),
    }
}

/// What stops a CREATE: the name becomes a node and a folder, and a symbol is how a note finds its sound.
#[must_use]
pub fn rack_issues(name: &str, pads: &[RackPad], taken: &[String], has_rack: bool) -> Vec<RackIssue> {
    let mut out = Vec::new();
    if !has_rack {
        out.push(with("rb.noRack", "module", RACK_MODULE));
    }
    let shape = |s: &str| {
        let mut cs = s.chars();
        cs.next().is_some_and(|c| c.is_ascii_alphanumeric())
            && cs.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    };
    if name.is_empty() {
        out.push(issue("rb.noName"));
    } else if !shape(name) {
        out.push(issue("rb.nameShape"));
    }
    if taken.iter().any(|t| t == name) {
        out.push(with("rb.exists", "name", name));
    }
    if pads.is_empty() {
        out.push(issue("rb.noSample"));
    }
    let mut seen: Vec<&str> = Vec::new();
    for p in pads {
        let ok = !p.symbol.is_empty()
            && p.symbol
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !ok {
            out.push(with("rb.badSymbol", "symbol", &p.symbol));
        } else if seen.contains(&p.symbol.as_str()) {
            out.push(with("rb.dupSymbol", "symbol", &p.symbol));
        }
        seen.push(&p.symbol);
    }
    let mut files: Vec<&str> = Vec::new();
    for p in pads {
        if files.contains(&p.file_name.as_str()) {
            out.push(with("rb.samePad", "file", &p.file_name));
        }
        files.push(&p.file_name);
    }
    out
}

/// The KIT node the builder writes, as k7x: a node of the sampler, one tap per pad, one per line, in the order given.
///
/// # Errors
/// A pad number with no k7x spelling.
pub fn build_kit_node(
    name: &str,
    pads: &[RackPad],
    dest: &KitDestination,
    rack_ref: &str,
) -> Result<String, EditError> {
    let symbols = Value::object(
        pads.iter()
            .map(|p| {
                let tap = Value::Map(vec![
                    ("note".to_owned(), Value::Num(p.note)),
                    (
                        "sample".to_owned(),
                        Value::Str(kit_sample_ref(dest, name, &p.file_name)),
                    ),
                ]);
                (p.symbol.clone(), tap)
            })
            .collect(),
    );
    let node = Value::Map(vec![
        ("extends".to_owned(), Value::Str(rack_ref.to_owned())),
        ("engine".to_owned(), Value::Str(RACK_ENGINE.to_owned())),
        ("rack".to_owned(), Value::Map(vec![("symbols".to_owned(), symbols)])),
    ]);
    let header = [format!(
        "{name} — a kit, built with the rack builder: each tap a note and the sample it fires."
    )];
    // `BD: { note: 36, sample: … }`: a kit is read down, the way a drum map is.
    let inline = |path: &[PathSeg], _: &Value| {
        path.len() == 3 && path[0] == PathSeg::Key("rack".into()) && path[1] == PathSeg::Key("symbols".into())
    };
    stringify(
        &node,
        &StringifyOptions {
            header: &header,
            inline: Some(&inline),
        },
    )
}
