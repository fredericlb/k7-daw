// doc/texts.rs — the files a plugin names, read through the Source once per pass: rack paths and jsdsp graphs.
// A reader would get wrong that a refused file costs that file only: it is a WARNING on the module, never a failure of the compile.

use super::notes::Notes;
use crate::files::{Finding, Severity};
use crate::jsdsp::{GraphState, JsDspGraph, jsdsp_graph};
use crate::modules::assets::Places;
use crate::roots::{is_absolute, spell};
use crate::source::path::join;
use crate::source::{Source, SourceError};
use std::collections::HashMap;
use std::sync::Arc;

/// What a rack path became: kept as spelled, or refused (outside the mounts, or no path at all).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum RackPath {
    Kept(String),
    Outside,
    Empty,
}

/// The reads of one pass, each memoised by its path.
pub(super) struct Texts<'a> {
    source: &'a dyn Source,
    project: &'a str,
    places: &'a Places,
    racks: HashMap<String, RackPath>,
    graphs: HashMap<String, Arc<JsDspGraph>>,
}

impl<'a> Texts<'a> {
    pub fn new(source: &'a dyn Source, project: &'a str, places: &'a Places) -> Texts<'a> {
        Texts {
            source,
            project,
            places,
            racks: HashMap::new(),
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
}

/// What a DSP module's graph says of it: nothing when it plays, else one ERROR in the TypeScript's own sentence.
pub fn jsdsp_finding(g: &JsDspGraph, label: &str, name: &str) -> Option<Finding> {
    let reason = match &g.state {
        GraphState::Fresh => return None,
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
