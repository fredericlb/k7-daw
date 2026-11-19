//! docpaths.rs — the path fields of a `ProjectDoc` a browser posts, each mapped to a real path and confined (3f-5).
//! What a reader gets wrong: the walk names the fields of the share's `rehome` (shareSnapshot.ts), so a field added to the doc
//! that names a file must be added here too, or it reaches the engine unjudged; docs/verb-host.md.

use song::clips::ClipDoc;
use song::doc::{PluginDoc, ProjectDoc};

use super::Error;
use super::gate::{forbidden, read_path, within};
use crate::verb_paths::Virtual;

/// Maps one path field: absolute paths are virtual and go through the gate; a relative one needs the render's base.
pub struct Mapper<'a> {
    pub virt: &'a Virtual,
    /// The render's base folder, already real and judged: a relative path resolves under it.
    pub base: Option<&'a str>,
}

impl Mapper<'_> {
    /// The real path of a field; an empty one names nothing and stays empty.
    pub fn field(&self, shown: &str) -> Result<String, Error> {
        if shown.is_empty() {
            return Ok(String::new());
        }
        if shown.starts_with('/') {
            return read_path(self.virt, shown);
        }
        let Some(base) = self.base else {
            return Err(forbidden(shown));
        };
        let climbs = shown.split('/').any(|s| s == "..");
        if climbs || shown.contains(['\0', '\\']) || shown.as_bytes().get(1) == Some(&b':') {
            return Err(forbidden(shown));
        }
        within(self.virt, &crate::paths::join(&[base, shown]), shown)
    }
}

fn clip(m: &Mapper, c: &mut ClipDoc) -> Result<(), Error> {
    c.source = m.field(&c.source)?;
    Ok(())
}

fn plugin(m: &Mapper, p: &mut PluginDoc) -> Result<(), Error> {
    p.path = m.field(&p.path)?;
    p.preset_save = m.field(&p.preset_save)?;
    for v in [
        &mut p.ir_file,
        &mut p.nam_file,
        &mut p.surge_patch,
        &mut p.sfz_file,
        &mut p.deck_file,
    ]
    .into_iter()
    .flatten()
    {
        *v = m.field(v)?;
    }
    for b in p.pattern_banks.iter_mut().flatten() {
        *b = m.field(b)?;
    }
    for r in p.rack.iter_mut().flatten() {
        r.clip = m.field(&r.clip)?;
    }
    Ok(())
}

/// Every path field of `doc` made real, or the 403 of the first one outside the folders the person reads.
pub fn map_doc(m: &Mapper, doc: &mut ProjectDoc) -> Result<(), Error> {
    doc.record_dir = m.field(&doc.record_dir)?;
    for p in &mut doc.master_plugins {
        plugin(m, p)?;
    }
    for t in &mut doc.tracks {
        for c in t
            .clips
            .iter_mut()
            .chain(t.frozen_material.iter_mut().flatten())
        {
            clip(m, c)?;
        }
        for p in &mut t.plugins {
            plugin(m, p)?;
        }
    }
    for lane in doc
        .timelines
        .iter_mut()
        .flatten()
        .flat_map(|t| t.lanes.iter_mut())
    {
        for c in &mut lane.clips {
            clip(m, c)?;
        }
    }
    Ok(())
}
