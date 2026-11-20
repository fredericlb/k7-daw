// doc/freeze_points.rs — the frozen points in force: what a render silences, and which track plays it.
// A reader would get wrong that a render id carries a RESERVED suffix: the compiled document has no other way to say "this is a render".

use super::notes::{Notes, finding};
use crate::clips::{Budget, ClipDoc, ClipKind};
use crate::files::Severity;
use crate::freeze::{Edges, FreezePlan, freeze_plans, name_units, render_ready};
use crate::lanes::inputs::Inputs;
use crate::lanes::ownership::Ownership;
use crate::roots::{is_absolute, spell};
use crate::source::path::{PathRule, join};
use crate::source::{Source, SourceError};
use std::collections::{HashMap, HashSet};

/// What a render's clip id ends with.
pub const FROZEN_CLIP_SUFFIX: &str = "__frozen";

/// The one clip a frozen track plays; `length: 0` tells the engine to use the file's natural length.
pub fn frozen_clip(id: &str, source: &str) -> ClipDoc {
    ClipDoc {
        id: format!("{id}{FROZEN_CLIP_SUFFIX}"),
        kind: ClipKind::Audio,
        name: format!("{id} (frozen)"),
        source: source.to_owned(),
        start: 0.0,
        length: 0.0,
        offset: 0.0,
        source_from: None,
        source_to: None,
        fade_in: 0.0,
        fade_out: 0.0,
        gain_db: None,
        loop_start: 0.0,
        loop_end: 0.0,
        notes: Vec::new(),
        warp: None,
        warp_off: None,
        disabled: None,
        event: None,
        comp: None,
        crossfade: None,
        nest: None,
    }
}

/// The frozen points in force.
#[derive(Debug, Default)]
pub(super) struct FreezePoints {
    pub plans: Vec<FreezePlan>,
    pub lanes: HashSet<String>,
    pub plugins: HashSet<String>,
    /// Track → the render it plays, as the Source spells it.
    pub render_on: HashMap<String, String>,
    /// The freeze budget ran out: no plan is in force.
    pub capped: bool,
}

/// A render's path as the Source spells it, when it is there (decision 335); `Missing` is the Err.
fn render_at(
    source: &dyn Source,
    project: &str,
    render: &str,
    manual: Option<&str>,
    notes: &mut Notes,
    rule: PathRule,
) -> Result<Option<String>, SourceError> {
    let abs = if is_absolute(render, rule) {
        render.to_owned()
    } else {
        join(project, render)
    };
    let Ok(at) = spell(&abs, rule) else { return Ok(None) };
    if render_ready(source, &at, rule)? {
        return Ok(Some(at));
    }
    if let Some(label) = manual
        && matches!(source.stat(&at), Err(SourceError::Outside))
    {
        notes.push(finding(
            label,
            Severity::Warning,
            "this module's render lies outside the project's folders, so it is not in force",
        ));
    }
    Ok(None)
}

/// The plans really in force over the modules `names` (decision 334); `auto` lays automatic renders over them.
#[allow(clippy::too_many_arguments)]
pub(super) fn freeze_points(
    source: &dyn Source,
    inputs: &Inputs,
    own: &Ownership,
    edges: &Edges<'_>,
    names: &[&str],
    is_lane: &dyn Fn(&str) -> bool,
    auto: &HashMap<&str, &str>,
    notes: &mut Notes,
    budget: &mut Budget,
) -> Result<FreezePoints, SourceError> {
    let mut at: Vec<(String, String)> = Vec::new();
    let mut paths: HashMap<&str, String> = HashMap::new();
    if !budget.charge(names.len()) {
        return Ok(FreezePoints {
            capped: true,
            ..FreezePoints::default()
        });
    }
    for &name in names {
        let overlay = auto.get(name).copied();
        let own_frozen = inputs
            .module(name)
            .and_then(|m| m.authored.frozen.as_ref())
            .map(|f| f.0.as_str());
        let Some(render) = overlay.or(own_frozen).filter(|r| !r.is_empty()) else {
            continue;
        };
        if !budget.charge(4) {
            return Ok(FreezePoints {
                capped: true,
                ..FreezePoints::default()
            });
        }
        let label = inputs.module(name).map(|m| m.own_label());
        let manual = if overlay.is_some() { None } else { label.as_deref() };
        if let Some(path) = render_at(source, &inputs.project, render, manual, notes, inputs.rule)? {
            paths.insert(name, path);
            at.push((name.to_owned(), render.to_owned()));
        }
    }
    let Some(plans) = freeze_plans(edges, &at, is_lane, budget) else {
        return Ok(FreezePoints {
            capped: true,
            ..FreezePoints::default()
        });
    };
    let mut out = FreezePoints::default();
    let mut holders: HashMap<&str, Vec<&str>> = HashMap::new();
    if !plans.is_empty() {
        for (name, c) in &inputs.chains {
            for s in &c.use_steps {
                holders.entry(s.module.as_str()).or_default().push(name);
            }
        }
    }
    for p in &plans {
        if !budget.charge(1 + p.lanes.iter().chain(&p.modules).map(|n| name_units(n)).sum::<usize>()) {
            return Ok(FreezePoints {
                capped: true,
                ..FreezePoints::default()
            });
        }
        out.lanes.extend(p.lanes.iter().cloned());
        out.plugins.extend(p.modules.iter().cloned());
        let Some(render) = paths.get(p.at.as_str()) else {
            continue;
        };
        for chain in holders.get(p.at.as_str()).into_iter().flatten() {
            if let Some(owner) = own.owner_of(chain) {
                out.render_on.insert(owner.to_owned(), render.clone());
            }
        }
    }
    out.plans = plans;
    Ok(out)
}
