// doc/texts.rs — the files a plugin names, read through the Source once per pass: rack paths, Faust sources, jsdsp graphs.
// A reader would get wrong that a refused file costs that file only: it is a WARNING on the module, never a failure of the compile.

use super::notes::Notes;
use crate::files::{Finding, Severity};
use crate::jsdsp::{GraphState, JsDspGraph, jsdsp_graph};
use crate::modules::assets::{Files, Places};
use crate::roots::{is_absolute, spell};
use crate::source::path::join;
use crate::source::{Kind, Source, SourceError, Span};
use std::collections::HashMap;
use std::sync::Arc;

/// The largest Faust source a plugin carries (decision 317).
pub const MAX_FAUST: u64 = 1 << 20;

/// What a rack path became: kept as spelled, or refused (outside the mounts, or no path at all).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RackPath {
    Kept(String),
    Outside,
    Empty,
}

/// Why a Faust source is not sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refused {
    Outside,
    Large,
    NotText,
}

/// The reads of one pass, each memoised by its path.
pub(super) struct Texts<'a> {
    source: &'a dyn Source,
    project: &'a str,
    places: &'a Places,
    racks: HashMap<String, RackPath>,
    faust: HashMap<String, Result<Arc<str>, Refused>>,
    graphs: HashMap<String, Arc<JsDspGraph>>,
}

impl<'a> Texts<'a> {
    pub fn new(source: &'a dyn Source, project: &'a str, places: &'a Places) -> Texts<'a> {
        Texts {
            source,
            project,
            places,
            racks: HashMap::new(),
            faust: HashMap::new(),
            graphs: HashMap::new(),
        }
    }

    /// A rack file's path, stat'ed once per path (decision 316); `Missing` is the Err.
    pub fn rack_path(&mut self, p: &str) -> Result<RackPath, SourceError> {
        if p.is_empty() {
            return Ok(RackPath::Empty);
        }
        if let Some(hit) = self.racks.get(p) {
            return Ok(hit.clone());
        }
        let abs = if is_absolute(p, self.places.rule) {
            p.to_owned()
        } else {
            join(self.project, p)
        };
        let out = match spell(&abs, self.places.rule) {
            Err(_) => RackPath::Empty,
            Ok(at) => match self.source.stat(&at) {
                Ok(_) | Err(SourceError::NotFound | SourceError::Io(_)) => RackPath::Kept(at),
                Err(SourceError::Outside) => RackPath::Outside,
                Err(SourceError::Missing) => return Err(SourceError::Missing),
            },
        };
        self.racks.insert(p.to_owned(), out.clone());
        Ok(out)
    }

    /// The text of a Faust module, or None with a WARNING on `label` when it may not be sent (decision 317).
    pub fn faust_text(&mut self, path: &str, label: &str, notes: &mut Notes) -> Result<Option<Arc<str>>, SourceError> {
        if !self.faust.contains_key(path) {
            let read = self.read_faust(path)?;
            self.faust.insert(path.to_owned(), read);
        }
        match self.faust.get(path) {
            Some(Ok(text)) if !text.is_empty() => Ok(Some(Arc::clone(text))),
            Some(Err(why)) => {
                let reason = match why {
                    Refused::Outside => "outside the project's folders",
                    Refused::Large => "too large to send",
                    Refused::NotText => "not text",
                };
                notes.push(warn(
                    label,
                    format!("this module's Faust source is {reason}, so it plays as its default"),
                ));
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    fn read_faust(&self, path: &str) -> Result<Result<Arc<str>, Refused>, SourceError> {
        let empty: Result<Arc<str>, Refused> = Ok(Arc::from(""));
        let Ok(at) = spell(path, self.places.rule) else {
            return Ok(Err(Refused::Outside));
        };
        match self.source.stat(&at) {
            Ok(st) if st.kind != Kind::File => return Ok(empty),
            Ok(st) if st.len > MAX_FAUST => return Ok(Err(Refused::Large)),
            Ok(_) => {}
            Err(SourceError::NotFound | SourceError::Io(_)) => return Ok(empty),
            Err(SourceError::Outside) => return Ok(Err(Refused::Outside)),
            Err(SourceError::Missing) => return Err(SourceError::Missing),
        }
        let bytes = match self.source.read(&at, Span::All) {
            Ok(b) => b,
            Err(SourceError::NotFound | SourceError::Io(_)) => return Ok(empty),
            Err(SourceError::Outside) => return Ok(Err(Refused::Outside)),
            Err(SourceError::Missing) => return Err(SourceError::Missing),
        };
        if bytes.len() as u64 > MAX_FAUST {
            return Ok(Err(Refused::Large));
        }
        Ok(String::from_utf8(bytes).map(Arc::from).map_err(|_| Refused::NotText))
    }

    /// The graph of a jsdsp source, read once per pass; a missing or unreadable one is a finding on `label` (decision 350).
    pub fn graph(
        &mut self,
        src: &str,
        label: &str,
        name: &str,
        notes: &mut Notes,
    ) -> Result<Arc<JsDspGraph>, SourceError> {
        if !self.graphs.contains_key(src) {
            let g = jsdsp_graph(self.source, src)?;
            self.graphs.insert(src.to_owned(), Arc::new(g));
        }
        let g = Arc::clone(&self.graphs[src]);
        if let Some(f) = jsdsp_finding(&g, label, name) {
            notes.push(f);
        }
        Ok(g)
    }

    /// The file each `soundfile` slot plays (decision 318); a ref that escapes the project is left out.
    pub fn faust_file(&mut self, r: &str) -> Result<Option<String>, SourceError> {
        if r.starts_with('@') {
            let mut files = Files::new(self.source, self.places, "");
            for family in ["wavetables", "samples"] {
                if let Some(f) = files.library_file(family, r)? {
                    return Ok(Some(f));
                }
            }
            return Ok(Some(r.to_owned()));
        }
        let spelled = if is_absolute(r, self.places.rule) {
            r.to_owned()
        } else {
            join(self.project, r)
        };
        // The Source's `Outside` is the project roots' confinement, the workspace's modules and the libraries among them.
        let Ok(abs) = spell(&spelled, self.places.rule) else {
            return Ok(None);
        };
        match self.source.stat(&abs) {
            Ok(_) => Ok(Some(abs)),
            Err(SourceError::NotFound | SourceError::Io(_)) => Ok(Some(r.to_owned())),
            Err(SourceError::Outside) => Ok(None),
            Err(SourceError::Missing) => Err(SourceError::Missing),
        }
    }
}

/// What a DSP module's graph says of it: nothing when it plays, else one ERROR in the TypeScript's own sentence.
pub fn jsdsp_finding(g: &JsDspGraph, label: &str, name: &str) -> Option<Finding> {
    let reason = match &g.state {
        GraphState::Fresh | GraphState::Orphan => return None,
        GraphState::Missing => format!("its {} is missing", g.json_path.rsplit('/').next().unwrap_or("")),
        GraphState::Invalid(why) => why.clone(),
    };
    Some(err(
        label,
        format!("module \"{name}\": its DSP does not build ({reason}) — it passes the sound through untouched"),
    ))
}

fn warn(file: &str, message: String) -> Finding {
    Finding {
        file: file.to_owned(),
        line: 1,
        col: 1,
        message,
        severity: Severity::Warning,
    }
}

fn err(file: &str, message: String) -> Finding {
    Finding {
        severity: Severity::Error,
        ..warn(file, message)
    }
}
