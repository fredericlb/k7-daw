// lanes/references.rs — the reference check: what a project names that points at nothing, as faults and as findings on a file.
// A reader would get wrong that a fault carries no message: `key()` is what a test and the gate compare (259).
// Every event is checked ONCE and cycles are found once by components (261); no lookup is a walk (262).

use super::cycles::Sccs;
use super::inputs::Inputs;
use super::wiring::{WiringInput, check_chain_inputs};
use super::{Lane, Lanes};
use crate::clips::fixed_fields_of;
use crate::files::{Clip, Finding, Placement, Sequence, Severity};
use crate::modules::Absent;
use crate::read::k7x_base;
use crate::roots::spell;
use crate::source::path::join;
use crate::source::{Source, SourceError};
use std::collections::{HashMap, HashSet};

pub use super::fault::{Fault, FaultKind, Holder, MAX_FAULTS, MAX_HELD, held};

/// What the check found: the faults, and each as an ERROR finding at 1:1 on the file that holds it (271).
#[derive(Debug, Default)]
pub struct Checked {
    pub faults: Vec<Fault>,
    pub findings: Vec<Finding>,
}

/// A message is at most this many bytes (259).
const MAX_MESSAGE: usize = 300;
/// A word quoted from a file is cut to this many characters (259).
const MAX_QUOTE: usize = 40;

fn quote(s: &str) -> String {
    s.chars().take(MAX_QUOTE).collect()
}

/// The file a fault is named by (271).
fn file_of(inputs: &Inputs, fault: &Fault) -> String {
    let label = |name: &str| inputs.local.modules.get(name).map(|m| m.own_label());
    match &fault.holder {
        Holder::Track(t) => label(t).unwrap_or_else(|| format!("nodes/{t}.k7x")),
        Holder::Node(m) | Holder::Module(m) => label(m).unwrap_or_else(|| format!("{m}/module.k7x")),
        Holder::Timeline { id, .. } => {
            let at = inputs.timelines.values.iter().position(|(i, _)| i == id);
            at.and_then(|n| inputs.timelines.names.get(n))
                .map_or_else(|| format!("timelines/{id}.k7x"), |name| format!("timelines/{name}"))
        }
        Holder::Chain { name, .. } => label(name)
            .or_else(|| {
                inputs
                    .chains
                    .get(name)
                    .and_then(|c| c.use_steps.first())
                    .and_then(|s| label(&s.module))
            })
            .unwrap_or_else(|| {
                (if inputs.routing.chain_project {
                    "chains"
                } else {
                    "graph.k7x"
                })
                .to_owned()
            }),
        Holder::File(f) if f.is_empty() => "project.k7x".to_owned(),
        Holder::File(f) => f.clone(),
        Holder::Console => "project.k7x".to_owned(),
    }
}

fn message_of(fault: &Fault) -> String {
    let t = quote(&fault.target);
    let what = match fault.kind {
        FaultKind::ExtendsBroken => format!("extends '{t}', whose file does not read"),
        FaultKind::ExtendsUnknown => format!("extends unknown module '{t}'"),
        FaultKind::TimelineCycle => format!("timeline '{t}' is placed inside itself"),
        FaultKind::TimelineFixed => format!("timeline '{t}' is placed with a field its length fixes"),
        FaultKind::ClipBroken => format!("clip '{t}' failed to load"),
        FaultKind::SequenceBroken => format!("sequence '{t}' failed to load"),
        FaultKind::RefMissing => format!("clip/sequence '{t}' not found"),
        FaultKind::AudioOutside => format!("audio clip '{t}' names a file outside the project"),
        FaultKind::AudioMissing => format!("audio clip '{t}' file missing"),
        FaultKind::SequenceGain => format!("sequence '{t}' is placed with a gain"),
        FaultKind::LoopNoTo => format!("clip '{t}' is placed with a loop but no end"),
        FaultKind::TimelineBroken => "timeline failed to load".to_owned(),
        FaultKind::StrudelMissing => format!("strudel pattern '{t}' file missing"),
        FaultKind::GroupSource => format!("from '{t}' is a group — groups carry no signal"),
        FaultKind::TooFew => format!("feeds too few inputs to module '{t}'"),
        FaultKind::TooMany => format!("feeds more inputs than module '{t}' has ports"),
        FaultKind::PortType => format!("port '{t}' is fed a source of another type"),
        FaultKind::TapUnrouted => format!("tap '{t}' is routed to no subchain of its chain file"),
        FaultKind::ConsoleAbsent => format!("no {t} module there"),
        FaultKind::More => format!("more than {MAX_FAULTS} faults — the rest are left out"),
    };
    let mut message = format!("{} — {what}", fault.holder.words());
    while message.len() > MAX_MESSAGE {
        message.pop();
    }
    message
}

impl Holder {
    fn words(&self) -> String {
        match self {
            Holder::Track(t) => format!("track '{t}'"),
            Holder::Timeline { id, lane } => format!("timeline '{id}' lane '{lane}'"),
            Holder::Module(m) => format!("module '{m}'"),
            Holder::Chain { name, step: None } => format!("chain '{name}'"),
            Holder::Chain { name, step: Some(n) } => format!("chain '{name}' step {n}"),
            Holder::Node(n) => format!("node '{n}'"),
            Holder::File(f) => format!("file '{f}'"),
            Holder::Console => "project.console".to_owned(),
        }
    }
}

/// A fault as the ERROR finding that names its file (271).
pub fn finding_of(inputs: &Inputs, fault: &Fault) -> Finding {
    Finding {
        file: file_of(inputs, fault),
        line: 1,
        col: 1,
        message: message_of(fault),
        severity: Severity::Error,
    }
}

/// Where a fault's file is found when the walk knows the holder's full name, which a fault itself holds cut (271).
#[derive(Clone, Copy)]
enum Place<'p> {
    /// Read the file off the cut holder (`file_of`).
    Holder,
    /// The timeline at this position of `Inputs::timelines`.
    Timeline(usize),
    /// A lane's node, by its full name.
    Track(&'p str),
    /// A module, local or vocabulary, by its full name.
    Module(&'p str),
    /// A file named as it stands.
    Raw(&'p str),
}

/// The faults found so far, each distinct one once, with the finding that names its file, and the stop at `MAX_FAULTS` (260).
struct Collector<'a> {
    inputs: &'a Inputs,
    faults: Vec<Fault>,
    findings: Vec<Finding>,
    seen: HashSet<String>,
    full: bool,
}

impl Collector<'_> {
    fn file(&self, place: Place<'_>, fault: &Fault) -> String {
        let (inputs, label) = (self.inputs, |n: &str| {
            self.inputs.local.modules.get(n).map(|m| m.own_label())
        });
        match place {
            Place::Holder => file_of(inputs, fault),
            Place::Timeline(x) => match (inputs.timelines.names.get(x), inputs.timelines.values.get(x)) {
                (Some(name), _) => format!("timelines/{name}"),
                (None, Some((id, _))) => format!("timelines/{id}.k7x"),
                (None, None) => file_of(inputs, fault),
            },
            Place::Track(n) => label(n).unwrap_or_else(|| format!("nodes/{n}.k7x")),
            Place::Module(n) => label(n).unwrap_or_else(|| format!("{n}/module.k7x")),
            Place::Raw(f) => f.to_owned(),
        }
    }

    /// The family finding that says why a file did not read, for the fault that names it.
    fn cause(&self, fault: &Fault) -> Option<&Finding> {
        let inputs = self.inputs;
        let (found, dir) = match fault.kind {
            FaultKind::ClipBroken => (&inputs.clips.findings, "clips"),
            FaultKind::SequenceBroken => (&inputs.sequences.findings, "sequences"),
            FaultKind::TimelineBroken => {
                let Holder::File(f) = &fault.holder else { return None };
                return inputs
                    .timelines
                    .findings
                    .iter()
                    .find(|x| x.severity == Severity::Error && &x.file == f);
            }
            _ => return None,
        };
        found.iter().find(|f| {
            f.file
                .strip_prefix(dir)
                .and_then(|r| r.strip_prefix('/'))
                .and_then(k7x_base)
                .is_some_and(|b| quote(b) == fault.target)
        })
    }

    fn push(&mut self, fault: Fault, place: Place<'_>) {
        let mut file = self.file(place, &fault);
        let mut message = message_of(&fault);
        // The file that did not read, and what is wrong in it, as the TypeScript's sentence says them.
        if let Some(c) = self.cause(&fault) {
            file.clone_from(&c.file);
            message = format!("{message} — {}: {}", c.file, c.message);
            while message.len() > MAX_MESSAGE {
                message.pop();
            }
        }
        self.findings.push(Finding {
            file,
            line: 1,
            col: 1,
            message,
            severity: Severity::Error,
        });
        self.faults.push(fault);
    }

    fn stop(&mut self) {
        self.full = true;
        self.push(
            Fault::new(FaultKind::More, Holder::File(String::new()), ""),
            Place::Holder,
        );
    }

    fn add(&mut self, kind: FaultKind, holder: &Holder, target: &str, place: Place<'_>) {
        if self.full {
            return;
        }
        let fault = Fault::new(kind, holder, target);
        if !self.seen.insert(fault.key()) {
            return;
        }
        if self.faults.len() >= MAX_FAULTS {
            self.stop();
        } else {
            self.push(fault, place);
        }
    }
}

/// What the walk looks references up in: maps built once from the three families (262).
struct Walk<'a> {
    source: &'a dyn Source,
    inputs: &'a Inputs,
    clips: HashMap<&'a str, &'a Clip>,
    sequences: HashMap<&'a str, &'a Sequence>,
    timelines: HashMap<&'a str, usize>,
    broken_clips: HashSet<&'a str>,
    broken_sequences: HashSet<&'a str>,
    audio: HashMap<&'a str, Audio>,
    sccs: Sccs,
    out: Collector<'a>,
}

#[derive(Clone, Copy)]
enum Audio {
    Fine,
    Outside,
    Missing,
}

/// The ids of the family files that did not read, by the base their file name carries.
fn broken_ids<'a>(findings: &'a [Finding], dir: &str) -> HashSet<&'a str> {
    findings
        .iter()
        .filter_map(|f| f.file.strip_prefix(dir)?.strip_prefix('/'))
        .filter_map(k7x_base)
        .collect()
}

impl<'a> Walk<'a> {
    fn audio_state(&mut self, id: &'a str) -> Result<Audio, SourceError> {
        if let Some(a) = self.audio.get(id) {
            return Ok(*a);
        }
        let path = self.inputs.audio_paths.get(id).map_or("", String::as_str);
        let state = if path.is_empty() {
            Audio::Outside
        } else {
            match self.source.stat(path) {
                Ok(_) => Audio::Fine,
                Err(SourceError::NotFound | SourceError::Io(_)) => Audio::Missing,
                Err(SourceError::Outside) => Audio::Outside,
                Err(e) => return Err(e),
            }
        };
        self.audio.insert(id, state);
        Ok(state)
    }

    /// One event's checks, `holder` naming its lane and `within` the timeline it stands in (261).
    fn event(
        &mut self,
        holder: &Holder,
        place: Place<'_>,
        p: &'a Placement,
        within: Option<usize>,
    ) -> Result<(), SourceError> {
        if self.out.full {
            return Ok(());
        }
        let r = p.ref_.as_ref().map_or("", |r| r.0.as_str());
        if r.is_empty() {
            return Ok(());
        }
        if let Some(&to) = self.timelines.get(r) {
            let cycle = within.is_some_and(|x| self.sccs.same_scc(x, to));
            if cycle {
                self.out.add(FaultKind::TimelineCycle, holder, r, place);
            }
            if within != Some(to) && !fixed_fields_of(p).is_empty() {
                self.out.add(FaultKind::TimelineFixed, holder, r, place);
            }
            return Ok(());
        }
        let (clip, seq) = (self.clips.get(r).copied(), self.sequences.contains_key(r));
        match (clip, seq) {
            (None, false) => {
                let kind = if self.broken_clips.contains(r) {
                    FaultKind::ClipBroken
                } else if self.broken_sequences.contains(r) {
                    FaultKind::SequenceBroken
                } else {
                    FaultKind::RefMissing
                };
                self.out.add(kind, holder, r, place);
            }
            (Some(_), _) => match self.audio_state(r)? {
                Audio::Fine => {}
                Audio::Outside => self.out.add(FaultKind::AudioOutside, holder, r, place),
                Audio::Missing => self.out.add(FaultKind::AudioMissing, holder, r, place),
            },
            (None, true) => {}
        }
        // A trim is a CLIP gain, which only audio has: a MIDI event shares its lane's instrument.
        if clip.is_none() && seq && p.gain.as_ref().is_some_and(|g| g.0 != 0.0) {
            self.out.add(FaultKind::SequenceGain, holder, r, place);
        }
        // Repeating needs a known end: the clip's `to` or the event's own.
        let clip_has_end = matches!(clip, Some(Clip::Audio(a)) if a.to.is_some());
        if clip.is_some() && p.loop_.as_ref().is_some_and(|l| l.0 > 1) && !clip_has_end && p.to.is_none() {
            self.out.add(FaultKind::LoopNoTo, holder, r, place);
        }
        Ok(())
    }
}

/// Unresolved references and miswired ports (docs/nodeProject.md), each one fault and not a throw (270).
pub fn check_references(source: &dyn Source, inputs: &Inputs, lanes: &Lanes<'_>) -> Result<Checked, SourceError> {
    let tl = &inputs.timelines.values;
    let timelines: HashMap<&str, usize> = tl.iter().enumerate().map(|(n, (id, _))| (id.as_str(), n)).collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); tl.len()];
    for (x, (_, t)) in tl.iter().enumerate() {
        for e in t.lanes.iter().flat_map(|l| l.events.iter().flatten()) {
            if let Some(&y) = e.ref_.as_ref().and_then(|r| timelines.get(r.0.as_str())) {
                adj[x].push(y);
            }
        }
    }
    let mut w = Walk {
        source,
        inputs,
        clips: inputs.clips.values.iter().map(|(id, c)| (id.as_str(), &**c)).collect(),
        sequences: inputs
            .sequences
            .values
            .iter()
            .map(|(id, s)| (id.as_str(), &**s))
            .collect(),
        timelines,
        broken_clips: broken_ids(&inputs.clips.findings, "clips"),
        broken_sequences: broken_ids(&inputs.sequences.findings, "sequences"),
        audio: HashMap::new(),
        sccs: Sccs::of(&adj),
        out: Collector {
            inputs,
            faults: Vec::new(),
            findings: Vec::new(),
            seen: HashSet::new(),
            full: false,
        },
    };
    extends_faults(&mut w);
    for (name, lane) in &lanes.sound {
        let holder = Holder::Track(held(name));
        for p in lane.events {
            w.event(&holder, Place::Track(name), p, None)?;
        }
    }
    for (x, (id, t)) in tl.iter().enumerate() {
        for l in &t.lanes {
            let holder = Holder::Timeline {
                id: held(id),
                lane: held(&l.lane.0),
            };
            for p in l.events.iter().flatten() {
                w.event(&holder, Place::Timeline(x), p, Some(x))?;
            }
        }
    }
    for f in inputs
        .timelines
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
    {
        let holder = Holder::File(held(&f.file));
        w.out.add(FaultKind::TimelineBroken, &holder, "", Place::Raw(&f.file));
    }
    strudel_faults(&mut w, lanes)?;
    wiring_faults(&mut w, lanes);
    Ok(Checked {
        faults: w.out.faults,
        findings: w.out.findings,
    })
}

/// A step's module that extends a ref no module answers to: broken when its file did not read, else unknown (266).
fn extends_faults(w: &mut Walk<'_>) {
    let mut seen: HashSet<&str> = HashSet::new();
    for c in w.inputs.chains.values() {
        for step in &c.use_steps {
            let name = step.module.as_str();
            if !seen.insert(name) {
                continue;
            }
            // A missing module is NOT fatal: it compiles to nothing, and the graph draws a `?`.
            let Some(m) = w.inputs.module(name) else { continue };
            let ext = m.authored.extends.as_deref().unwrap_or_default();
            if ext.is_empty() || w.inputs.vocabulary.modules.contains_key(ext) {
                continue;
            }
            let kind = match w.inputs.vocabulary.absent(ext) {
                Absent::Broken { .. } => FaultKind::ExtendsBroken,
                Absent::Unknown => FaultKind::ExtendsUnknown,
            };
            w.out.add(kind, &Holder::Module(held(name)), ext, Place::Module(name));
        }
    }
}

/// The pattern file must exist; a path the Source refuses is `read_pattern`'s warning, not a missing file (264).
fn strudel_faults(w: &mut Walk<'_>, lanes: &Lanes<'_>) -> Result<(), SourceError> {
    for (name, lane) in &lanes.sound {
        let Some(strudel) = lane.strudel else { continue };
        let Ok(path) = spell(&join(&w.inputs.project, strudel), w.inputs.rule) else {
            continue;
        };
        match w.source.stat(&path) {
            Ok(_) | Err(SourceError::Outside) => {}
            Err(SourceError::NotFound | SourceError::Io(_)) => {
                let holder = Holder::Track(held(name));
                w.out
                    .add(FaultKind::StrudelMissing, &holder, strudel, Place::Track(name));
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn wiring_faults(w: &mut Walk<'_>, lanes: &Lanes<'_>) {
    let inputs = w.inputs;
    let tap_fed: HashSet<String> = inputs
        .routing
        .segments
        .tap_feeds_of()
        .iter()
        .map(|f| f.to.clone())
        .collect();
    let inputs_of = |name: &str| inputs.module(name).map(|m| m.inputs());
    let picture = |n: &str| lanes.picture.contains_key(n);
    let sound: &indexmap::IndexMap<&str, Lane<'_>> = &lanes.sound;
    let wiring = WiringInput {
        chains: &inputs.chains,
        inputs_of: &inputs_of,
        sound,
        clips: &w.clips,
        sequences: &w.sequences,
        picture: &picture,
        tap_fed: &tap_fed,
    };
    for f in check_chain_inputs(&wiring) {
        if f.kind == FaultKind::More {
            if !w.out.full {
                w.out.stop();
            }
            return;
        }
        w.out.add(f.kind, &f.holder, &f.target, Place::Holder);
    }
}
