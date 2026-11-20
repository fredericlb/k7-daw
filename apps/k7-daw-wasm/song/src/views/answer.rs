// views/answer.rs — one call that gives a host every view it asked for: the compile, the web project, the errors, the lists.
// A reader would get wrong that a view exists whether or not the compile refused: hints, surfaces and ports read the files alone.

use super::catalogue::{BaseModuleRef, CatalogueModule, Listed, base_modules, module_catalogue};
use super::errors::{ProjectErrors, project_errors};
use super::hints::{HintsAnswer, hints_of};
use super::io_ports::{IoPorts, io_ports};
use super::lengths::{ByHeader, Lengths};
use super::surfaces::{ProjectSurfaces, project_surfaces};
use super::web::{WebProject, web_project_of};
use crate::cache::Cache;
use crate::clips::ClipKind;
use crate::doc::{Compiled, Failure, ProjectDoc, Request, compile_keeping};
use crate::files::Finding;
use crate::lanes::inputs::{Inputs, load_inputs};
use crate::roots::spell;
use crate::source::{Source, SourceError};

/// Which views a call wants; a view not asked for costs nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Want {
    pub engine: bool,
    pub web: bool,
    pub errors: bool,
    pub hints: bool,
    pub surfaces: bool,
    pub io_ports: bool,
    pub catalogue: bool,
    pub base_modules: bool,
}

/// The browser's project, the lengths measured to draw it, and the one WARNING of a measure that hit its limit.
#[derive(Clone, Debug)]
pub struct WebAnswer {
    pub web: WebProject,
    pub lengths: Lengths,
    pub findings: Vec<Finding>,
}

/// What a call gave, a field to each view asked for.
#[derive(Debug, Default)]
pub struct Views {
    pub compiled: Option<Compiled>,
    pub web: Option<Result<WebAnswer, Failure>>,
    pub errors: Option<ProjectErrors>,
    pub hints: Option<HintsAnswer>,
    pub surfaces: Option<ProjectSurfaces>,
    pub io_ports: Option<IoPorts>,
    pub catalogue: Option<Listed<CatalogueModule>>,
    pub base_modules: Option<Listed<BaseModuleRef>>,
}

/// Every audio source a document plays or draws, in track order.
fn audio_sources(doc: &ProjectDoc) -> impl Iterator<Item = &str> {
    let tracks = doc.tracks.iter();
    let clips = tracks.flat_map(|t| t.clips.iter().chain(t.frozen_material.iter().flatten()));
    let lanes = doc
        .timelines
        .iter()
        .flatten()
        .flat_map(|t| t.lanes.iter().flat_map(|l| l.clips.iter()));
    clips
        .chain(lanes)
        .filter(|c| c.kind == ClipKind::Audio && !c.source.is_empty())
        .map(|c| c.source.as_str())
}

/// The web view: every length measured once, the first time, so a draft that places a clip finds its length (decision 612).
fn web_view(
    source: &dyn Source,
    cache: &mut Cache,
    doc: &ProjectDoc,
    inputs: &Inputs,
) -> Result<WebAnswer, SourceError> {
    let mut by = ByHeader::new(source, cache);
    by.measure_all(
        inputs
            .audio_paths
            .values()
            .filter(|p| !p.is_empty())
            .map(String::as_str),
    )?;
    by.measure_all(audio_sources(doc))?;
    let web = web_project_of(doc, &mut by)?;
    let (lengths, findings) = by.finish();
    Ok(WebAnswer { web, lengths, findings })
}

/// The compile's reads, taken the first time a view asks for them.
fn held<'a>(
    slot: &'a mut Option<Inputs>,
    source: &dyn Source,
    cache: &mut Cache,
    req: &Request,
) -> Result<&'a Inputs, SourceError> {
    if slot.is_none() {
        *slot = Some(load_inputs(source, cache, &req.roots, req.rig.clone())?);
    }
    slot.as_ref().ok_or(SourceError::Missing)
}

/// The views `want` names, from one compile and one read of the project (decision 642); `Err` is `Missing` alone.
pub fn answer(source: &dyn Source, cache: &mut Cache, req: &Request, want: &Want) -> Result<Views, SourceError> {
    let project = spell(&req.roots.project, req.roots.paths)?;
    let mut views = Views::default();
    let mut inputs: Option<Inputs> = None;
    let mut compiled = None;
    if want.engine || want.web || want.errors {
        let (made, kept) = compile_keeping(source, cache, req)?;
        compiled = Some(made);
        inputs = kept;
    }
    if let Some(c) = &compiled {
        match &c.engine {
            Err(f) if want.web => views.web = Some(Err(f.clone())),
            Ok(doc) if want.web => {
                let i = held(&mut inputs, source, cache, req)?;
                views.web = Some(Ok(web_view(source, cache, doc, i)?));
            }
            _ => {}
        }
        if want.errors {
            if c.engine.is_ok() {
                held(&mut inputs, source, cache, req)?;
            }
            views.errors = Some(project_errors(source, cache, req, c, inputs.as_ref())?);
        }
    }
    if want.surfaces {
        views.surfaces = Some(project_surfaces(source, cache, &project)?);
    }
    if want.hints || want.io_ports || want.catalogue || want.base_modules {
        let i = held(&mut inputs, source, cache, req)?;
        if want.hints {
            views.hints = Some(hints_of(source, cache, &req.roots, i)?);
        }
        if want.io_ports {
            views.io_ports = Some(io_ports(i));
        }
        if want.catalogue {
            views.catalogue = Some(module_catalogue(source, &req.roots, &i.vocabulary)?);
        }
        if want.base_modules {
            views.base_modules = Some(base_modules(&i.vocabulary));
        }
    }
    if want.engine {
        views.compiled = compiled;
    }
    Ok(views)
}
