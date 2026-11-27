// views/surfaces.rs — a `surfaces/<name>.k7x` decoded, and every surface a project declares.
// A reader would get wrong that `mapping: null` is refused but an ENTRY of `null` is a value: an unwired control.

use super::Diagnostic;
use super::nullable::Nullable;
use crate::cache::Cache;
use crate::files::{Check, DeError, Finding, Severity, decode_with, from_map};
use crate::jsorder::pair_cmp;
use crate::lanes::inputs::Inputs;
use crate::read::read_family;
use crate::source::path::join;
use crate::source::{Source, SourceError};
use indexmap::IndexMap;
use k7x::PathSeg;
use serde::{Deserialize, Serialize};

/// The folder a project keeps its surfaces in.
pub const SURFACES_DIR: &str = "surfaces";

/// The bytes a finding's message keeps when it quotes a name.
const MESSAGE_CUT: usize = 280;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum SurfaceType {
    Turntables,
    Timeline,
    Viewport,
    Mastering,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum SurfaceLayout {
    View,
    Split,
    Floating,
}

/// The kinds of surface there are, by name: one nothing can draw is a blank screen.
pub const SURFACE_TYPES: [&str; 4] = ["turntables", "timeline", "viewport", "mastering"];

impl SurfaceType {
    fn name(self) -> &'static str {
        match self {
            SurfaceType::Turntables => "turntables",
            SurfaceType::Timeline => "timeline",
            SurfaceType::Viewport => "viewport",
            SurfaceType::Mastering => "mastering",
        }
    }

    /// The layouts a type can be drawn in today.
    fn layouts(self) -> &'static [SurfaceLayout] {
        match self {
            SurfaceType::Viewport => &[SurfaceLayout::Floating],
            _ => &[SurfaceLayout::Split],
        }
    }
}

impl SurfaceLayout {
    fn name(self) -> &'static str {
        match self {
            SurfaceLayout::View => "view",
            SurfaceLayout::Split => "split",
            SurfaceLayout::Floating => "floating",
        }
    }
}

/// A module and one of its params a control drives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct BindTarget {
    pub module: String,
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub invert: Option<bool>,
}

/// What one control drives: a target, and at most one second.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct ViewBinding {
    pub module: String,
    pub param: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub invert: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub also: Option<BindTarget>,
}

/// A surface file: what to draw, where, and what its components move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct SurfaceFile {
    #[serde(rename = "type")]
    #[cfg_attr(feature = "ts", ts(rename = "type"))]
    pub kind: SurfaceType,
    pub layout: SurfaceLayout,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub playlist: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stems: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional, type = "{ [key in string]: import(\"./ViewBinding\").ViewBinding | null }"))]
    pub mapping: Option<IndexMap<String, Nullable<ViewBinding>>>,
}

fn key(name: &str) -> PathSeg {
    PathSeg::Key(name.to_owned())
}

impl Check for SurfaceFile {
    fn check(&self) -> Option<(Vec<PathSeg>, String)> {
        // A control id is a non-empty key, which the schema checks before any refinement runs.
        if let Some(m) = &self.mapping
            && m.contains_key("")
        {
            return Some((vec![key("mapping"), key("")], "expected a non-empty key".to_owned()));
        }
        for (id, bound) in self.mapping.iter().flatten() {
            let Some(b) = &bound.0 else { continue };
            for (field, text) in [("module", &b.module), ("param", &b.param)] {
                if text.is_empty() {
                    return Some((
                        vec![key("mapping"), key(id), key(field)],
                        "expected a non-empty string".to_owned(),
                    ));
                }
            }
            if let Some(a) = &b.also {
                for (field, text) in [("module", &a.module), ("param", &a.param)] {
                    if text.is_empty() {
                        let at = vec![key("mapping"), key(id), key("also"), key(field)];
                        return Some((at, "expected a non-empty string".to_owned()));
                    }
                }
            }
        }
        if self.playlist.as_deref() == Some("") {
            return Some((vec![key("playlist")], "expected a non-empty string".to_owned()));
        }
        let allowed = self.kind.layouts();
        if !allowed.contains(&self.layout) {
            let list: Vec<String> = allowed.iter().map(|l| format!("'{}'", l.name())).collect();
            return Some((
                vec![key("layout")],
                format!(
                    "a '{}' surface draws in {}, not in '{}'",
                    self.kind.name(),
                    list.join(" or "),
                    self.layout.name()
                ),
            ));
        }
        if self.playlist.is_some() && self.kind != SurfaceType::Turntables {
            return Some((
                vec![key("playlist")],
                format!(
                    "only a 'turntables' surface reads a playlist — a '{}' draws no song browser",
                    self.kind.name()
                ),
            ));
        }
        if self.stems.is_some() && self.kind != SurfaceType::Turntables {
            return Some((
                vec![key("stems")],
                format!(
                    "only a 'turntables' surface separates a record — a '{}' loads none",
                    self.kind.name()
                ),
            ));
        }
        None
    }
}

/// A surface file's text decoded; an empty one says what a surface must state.
pub fn decode_surface(file: &str, text: &str) -> Result<SurfaceFile, Finding> {
    let decoded = decode_with(file, text, |root| {
        if root.entries.is_empty() {
            return Err(DeError {
                message: "a surface file says what it is: `type` (turntables, timeline, viewport, mastering) \
                          and `layout` (view, split, floating). This one is empty."
                    .to_owned(),
                at: Some(root.start),
            });
        }
        from_map::<SurfaceFile>(root)
    });
    decoded.map_err(shorten)
}

/// A message that quotes a name is cut to `MESSAGE_CUT` bytes, on a character.
fn shorten(mut f: Finding) -> Finding {
    if f.message.len() > MESSAGE_CUT {
        let mut at = MESSAGE_CUT;
        while !f.message.is_char_boundary(at) {
            at -= 1;
        }
        f.message.truncate(at);
        f.message.push('…');
    }
    f
}

/// One surface as a reader gets it: its id is its file name, its file project-relative.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSurface {
    pub id: String,
    pub file: String,
    pub surface: SurfaceFile,
}

/// Every surface a project declares, plus whatever would not read.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ProjectSurfaces {
    pub surfaces: Vec<ProjectSurface>,
    pub diagnostics: Vec<Diagnostic>,
}

fn at_start(file: &str, message: String, severity: Severity) -> Diagnostic {
    Diagnostic {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message,
        severity,
    }
}

/// Every surface a project declares, in the order of `[base, name]` pairs sorted as strings; a file that will not read costs that file only.
pub fn project_surfaces(source: &dyn Source, cache: &mut Cache, project: &str) -> Result<ProjectSurfaces, SourceError> {
    let fam = match read_family::<SurfaceFile>(source, cache, project) {
        Ok(f) => f,
        Err(SourceError::Outside) => {
            let message = "outside the project's folders — skipped".to_owned();
            return Ok(ProjectSurfaces {
                surfaces: Vec::new(),
                diagnostics: vec![at_start(SURFACES_DIR, message, Severity::Warning)],
            });
        }
        Err(e) => return Err(e),
    };
    // The TypeScript sorts `[base, name]` pairs as strings, so "10" comes before "9" here although `read_family` lists 9 first.
    let mut shown: Vec<(ProjectSurface, &String)> = fam
        .values
        .iter()
        .zip(&fam.names)
        .map(|((id, surface), name)| {
            let shown = ProjectSurface {
                id: id.clone(),
                file: format!("{SURFACES_DIR}/{name}"),
                surface: (**surface).clone(),
            };
            (shown, name)
        })
        .collect();
    shown.sort_by(|a, b| pair_cmp((&a.0.id, a.1), (&b.0.id, b.1)));
    let surfaces = shown.into_iter().map(|(s, _)| s).collect();
    Ok(ProjectSurfaces {
        surfaces,
        diagnostics: fam.findings.iter().map(Diagnostic::from).collect(),
    })
}

/// Whether a folder of the project is there for the timeline to draw: a link out counts, as `existsSync` does.
fn is_there(source: &dyn Source, path: &str) -> Result<bool, SourceError> {
    match source.stat(path) {
        Ok(_) | Err(SourceError::Outside) => Ok(true),
        Err(SourceError::NotFound | SourceError::Io(_)) => Ok(false),
        Err(e) => Err(e),
    }
}

/// The two sentences `surfaces.md` fixes: what a project holding material or a picture lane still lacks.
fn missing_surface(message: &str) -> Finding {
    Finding {
        file: format!("{SURFACES_DIR}/"),
        line: 1,
        col: 1,
        message: message.to_owned(),
        severity: Severity::Warning,
    }
}

/// What a project's surfaces lack (decision 606): the read findings, then a timeline and a viewport owed.
pub fn surface_issues(source: &dyn Source, inputs: &Inputs, s: &ProjectSurfaces) -> Result<Vec<Finding>, SourceError> {
    let mut out: Vec<Finding> = s
        .diagnostics
        .iter()
        .map(|d| Finding {
            file: d.file.clone(),
            line: d.line,
            col: d.col,
            message: d.message.clone(),
            severity: d.severity,
        })
        .collect();
    let declares = |kind: SurfaceType| s.surfaces.iter().any(|p| p.surface.kind == kind);
    let video = !inputs.lanes().picture.is_empty();
    let material = video
        || is_there(source, &join(&inputs.project, "clips"))?
        || is_there(source, &join(&inputs.project, "sequences"))?;
    if material && !declares(SurfaceType::Timeline) {
        out.push(missing_surface(
            "this project is an arrangement and declares no timeline, so nothing draws its lanes — \
             run `project surface new timeline` (apps/k7-daw-frontend/packages/song/docs/surfaces.md)",
        ));
    }
    if video && !declares(SurfaceType::Viewport) {
        out.push(missing_surface(
            "this project has video lanes and declares no viewport, so nothing composites them — \
             run `project surface new viewport` (apps/k7-daw-frontend/packages/song/docs/surfaces.md)",
        ));
    }
    Ok(out)
}
