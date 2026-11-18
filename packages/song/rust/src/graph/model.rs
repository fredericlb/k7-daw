// graph/model.rs — the values a graph is made of: lanes as the graph reads them, and a module normalised for validate and the view.
// A reader would get wrong that a `GraphModule` borrows its `Resolved` and copies only what is asked of it by index (413).

use crate::lanes::live::{MonitorMode, monitor_mode};
use crate::modules::types::module_emits;
use crate::modules::{Input, IoKind, ModuleKind, Port, PortType};
use crate::resolve::Resolved;

/// What a placement names: a clip, a sequence, a timeline, or nothing the project holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipKind {
    Audio,
    Midi,
    Timeline,
    Missing,
}

impl ClipKind {
    pub fn name(self) -> &'static str {
        match self {
            ClipKind::Audio => "audio",
            ClipKind::Midi => "midi",
            ClipKind::Timeline => "timeline",
            ClipKind::Missing => "missing",
        }
    }
}

/// The three kinds of sound lane the graph draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackType {
    Single,
    Group,
    Strudel,
}

/// One event of a lane as the graph keeps it: its ref, where it starts in beats, what it names.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GraphPlacement<'i> {
    pub ref_: &'i str,
    pub start: f64,
    pub kind: ClipKind,
}

/// A sound lane as the graph reads it, borrowed from the module it is.
#[derive(Clone, Debug)]
pub struct Track<'i> {
    pub name: &'i str,
    pub kind: TrackType,
    pub color: Option<&'i str>,
    pub mute: bool,
    pub placements: Vec<GraphPlacement<'i>>,
    pub tracks: &'i [String],
    pub strudel: Option<&'i str>,
    pub video_source: Option<&'i str>,
}

/// A picture lane as the graph and the timeline need it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoLane<'i> {
    pub color: Option<&'i str>,
    pub mute: bool,
}

/// A typed input port; mandatory unless the module says otherwise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputDef {
    pub name: String,
    pub kind: PortType,
    pub mandatory: bool,
}

/// A typed output port, offered and never required.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDef {
    pub name: String,
    pub kind: PortType,
}

/// A live input's channel as declared: a label, or the numbers that are whole and positive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Channel<'i> {
    Label(&'i str),
    Numbers(Vec<u32>),
}

/// A module normalised once: the fields validate and the view ask by index, the rest read off `m`.
#[derive(Clone, Debug)]
pub struct GraphModule<'i> {
    pub m: &'i Resolved,
    pub kind: ModuleKind,
    pub io: Option<IoKind>,
    pub extends: Option<&'i str>,
    pub category: Option<&'i str>,
    pub inputs: Vec<InputDef>,
    pub outputs: Vec<OutputDef>,
    pub emits: Option<PortType>,
    pub channel: Option<Channel<'i>>,
    pub monitor: MonitorMode,
    /// `(param, curve id, trigger)`, an entry with no param or no curve being dropped.
    pub automations: Vec<(&'i str, &'i str, Option<&'i str>)>,
}

/// The name an io role is spelled with in a module file.
pub fn io_name(io: IoKind) -> &'static str {
    match io {
        IoKind::Out => "out",
        IoKind::In => "in",
        IoKind::VideoOut => "video_out",
        IoKind::File => "file",
        IoKind::Camera => "camera",
        IoKind::Midi => "midi",
    }
}

fn non_empty(s: Option<&str>) -> Option<&str> {
    s.filter(|s| !s.is_empty())
}

/// A declared `input`, normalised and not resolved: numbers kept when whole and positive, a label as it stands (normInput).
fn channel_of(i: &Input) -> Option<Channel<'_>> {
    match i {
        Input::Label(l) => non_empty(Some(l)).map(Channel::Label),
        Input::Channels(list) => {
            let ch: Vec<u32> = list
                .iter()
                .filter_map(|n| u32::try_from(*n).ok())
                .filter(|n| *n > 0)
                .collect();
            (!ch.is_empty()).then_some(Channel::Numbers(ch))
        }
    }
}

/// `m` as the graph reads it (413).
pub fn graph_module(m: &Resolved) -> GraphModule<'_> {
    let a = &m.authored;
    let outputs: &[Port] = m.outputs();
    let io = a.io;
    GraphModule {
        m,
        kind: m.kind.unwrap_or(ModuleKind::Plugin),
        io,
        extends: non_empty(a.extends.as_deref()),
        category: non_empty(m.category.as_deref()),
        inputs: m
            .inputs()
            .iter()
            .map(|p| InputDef {
                name: p.name.clone(),
                kind: p.kind,
                mandatory: p.mandatory != Some(false),
            })
            .collect(),
        outputs: outputs
            .iter()
            .map(|p| OutputDef {
                name: p.name.clone(),
                kind: p.kind,
            })
            .collect(),
        emits: module_emits(non_empty(m.category.as_deref()), io.map(io_name), outputs),
        channel: a.channel.as_ref().and_then(channel_of),
        monitor: monitor_mode(a.monitor.as_ref()),
        automations: a
            .automations
            .iter()
            .flatten()
            .filter_map(|x| {
                let curve = crate::curves::curve_id_from_ref(&x.curve.0);
                (!x.param.0.is_empty() && !curve.is_empty()).then(|| {
                    (
                        x.param.0.as_str(),
                        curve,
                        x.trigger.as_ref().map(|t| t.0.as_str()).filter(|t| !t.is_empty()),
                    )
                })
            })
            .collect(),
    }
}
