// session/make.rs — the groups of views a session makes, each from one call of the views and serialized once.
// A reader would get wrong that a refused compile is silent: `engine`, `web` and `graph` answer a `failure`, the other views answer themselves.

use super::{Ask, View};
use crate::cache::Cache;
use crate::doc::{Compiled, ProjectDoc, Request};
use crate::freeze::AutoFreeze;
use crate::graph::{GraphError, load_compiled_graph};
use crate::lanes::rig::Rig;
use crate::readset::{ReadSet, Watch};
use crate::source::{Source, SourceError};
use crate::views::Diagnostic;
use crate::views::answer::{Want, answer};
use crate::views::errors::FormatTooNew;
use crate::views::lengths::Lengths;
use serde::Serialize;
use std::sync::Arc;

/// The views one call makes together.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Group {
    Core,
    Hints,
    Surfaces,
    IoPorts,
    Catalogue,
    BaseModules,
    Graph,
}

impl Group {
    pub(super) fn of(view: View) -> Group {
        match view {
            View::Engine | View::Web | View::Errors => Group::Core,
            View::Hints => Group::Hints,
            View::Surfaces => Group::Surfaces,
            View::IoPorts => Group::IoPorts,
            View::Catalogue => Group::Catalogue,
            View::BaseModules => Group::BaseModules,
            View::Graph => Group::Graph,
        }
    }
}

/// A group's answer: the token it was made at, what it read, and its views as text.
pub(super) struct Made {
    pub(super) rev: Option<String>,
    /// The serial of the compile a graph was drawn over; none for any other group.
    pub(super) over: Option<u64>,
    pub(super) set: ReadSet,
    views: Vec<(View, Arc<str>)>,
}

impl Made {
    pub(super) fn text(&self, view: View) -> Option<Arc<str>> {
        self.views.iter().find(|(v, _)| *v == view).map(|(_, t)| t.clone())
    }

    pub(super) fn weight(&self) -> usize {
        self.views.iter().map(|(_, t)| t.len()).sum::<usize>() + self.set.text_bytes()
    }
}

/// The compile group, which also keeps the compile itself for the graph and the lists the host acts on.
pub(super) struct Core {
    pub(super) made: Made,
    pub(super) compiled: Compiled,
    pub(super) used: Vec<String>,
    pub(super) lengths: Lengths,
    /// Which compile this is, as the session counts them: a graph drawn over another one is not this compile's graph.
    pub(super) serial: u64,
}

impl Core {
    /// The compile is held as the document it made, which measures about three times its text (docs/rustCompilerHost.md, 808).
    pub(super) fn compiled_weight(&self) -> usize {
        self.made.text(View::Engine).map_or(0, |t| t.len() * 3)
    }
}

pub(super) fn request_of(ask: &Ask) -> Request {
    Request {
        roots: ask.roots.clone(),
        rig: Rig::from_json(&ask.rig),
        vsts: ask.vsts,
        thawed: ask.thawed.clone(),
    }
}

fn text<T: Serialize + ?Sized>(value: &T) -> Result<Arc<str>, SourceError> {
    serde_json::to_string(value)
        .map(Arc::from)
        .map_err(|e| SourceError::Io(e.to_string()))
}

#[derive(Serialize)]
struct EngineOut<'a> {
    doc: &'a ProjectDoc,
    freezes: &'a [AutoFreeze],
}

#[derive(Serialize)]
struct Failure<'a> {
    diagnostics: Vec<Diagnostic>,
    #[serde(rename = "formatTooNew", skip_serializing_if = "Option::is_none")]
    format_too_new: Option<&'a FormatTooNew>,
}

#[derive(Serialize)]
struct FailureOut<'a> {
    failure: Failure<'a>,
}

fn failure(findings: &[crate::files::Finding], too_new: Option<&FormatTooNew>) -> Result<Arc<str>, SourceError> {
    text(&FailureOut {
        failure: Failure {
            diagnostics: findings.iter().map(Diagnostic::from).collect(),
            format_too_new: too_new,
        },
    })
}

/// The compile group: `engine`, `web` and `errors` from one compile of the project (decision 705).
pub(super) fn core(cache: &mut Cache, source: &dyn Source, ask: &Ask) -> Result<Core, SourceError> {
    let req = request_of(ask);
    let watch = Watch::new(source);
    let want = Want {
        engine: true,
        web: true,
        errors: true,
        ..Want::default()
    };
    let mut views = answer(&watch, cache, &req, &want)?;
    let set = watch.into_set();
    let no_answer = || SourceError::Io("the compile gave no answer".to_owned());
    let compiled = views.compiled.take().ok_or_else(no_answer)?;
    let errors = views.errors.take().ok_or_else(no_answer)?;
    let too_new = errors.format_too_new.as_ref();
    let engine = match &compiled.engine {
        Ok(doc) => text(&EngineOut {
            doc,
            freezes: &compiled.freezes,
        })?,
        Err(f) => failure(&f.diagnostics, too_new)?,
    };
    let (web, lengths) = match views.web.take().ok_or_else(no_answer)? {
        Ok(w) => (text(&w.web)?, w.lengths),
        Err(f) => (failure(&f.diagnostics, too_new)?, Lengths::new()),
    };
    let made = Made {
        rev: ask.rev.clone(),
        over: None,
        set,
        views: vec![(View::Engine, engine), (View::Web, web), (View::Errors, text(&errors)?)],
    };
    Ok(Core {
        made,
        used: compiled.renders_used.clone(),
        lengths,
        serial: 0,
        compiled,
    })
}

/// Any other group, made on its own: one call of the views each, the graph over the compile `core` holds (decision 705).
pub(super) fn other(
    cache: &mut Cache,
    source: &dyn Source,
    ask: &Ask,
    group: Group,
    core: Option<&Core>,
) -> Result<Made, SourceError> {
    let req = request_of(ask);
    let watch = Watch::new(source);
    let mut want = Want::default();
    let views = match group {
        Group::Hints => {
            want.hints = true;
            let v = answer(&watch, cache, &req, &want)?;
            vec![(View::Hints, text(&v.hints.map(|h| h.hints))?)]
        }
        Group::Surfaces => {
            want.surfaces = true;
            let v = answer(&watch, cache, &req, &want)?;
            vec![(View::Surfaces, text(&v.surfaces)?)]
        }
        Group::IoPorts => {
            want.io_ports = true;
            let v = answer(&watch, cache, &req, &want)?;
            vec![(View::IoPorts, text(&v.io_ports.map(|p| p.ports))?)]
        }
        Group::Catalogue => {
            want.catalogue = true;
            let v = answer(&watch, cache, &req, &want)?;
            vec![(View::Catalogue, text(&v.catalogue.map(|c| c.items))?)]
        }
        Group::BaseModules => {
            want.base_modules = true;
            let v = answer(&watch, cache, &req, &want)?;
            vec![(View::BaseModules, text(&v.base_modules.map(|b| b.items))?)]
        }
        Group::Graph => {
            // A project that does not compile still draws its own manual plans.
            let compiled = core.map(|c| &c.compiled).filter(|c| c.engine.is_ok());
            let drawn = load_compiled_graph(&watch, cache, &req.roots, req.rig.clone(), compiled, ask.folder);
            let out = match drawn {
                Ok(a) => text(&a.graph)?,
                Err(GraphError::Source(e)) => return Err(e),
                Err(GraphError::Refused(r)) => failure(&r.findings, None)?,
            };
            vec![(View::Graph, out)]
        }
        Group::Core => Vec::new(),
    };
    Ok(Made {
        rev: ask.rev.clone(),
        over: if group == Group::Graph {
            core.map(|c| c.serial)
        } else {
            None
        },
        set: watch.into_set(),
        views,
    })
}
