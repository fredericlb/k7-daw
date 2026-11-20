// lanes/fault.rs — a fault: a kind, a holder and a target, with no message; `key()` is what a test and the gate compare (259).
// A reader would get wrong that every string a fault holds is cut to 200 characters, so a hostile ref never costs its size.

use std::borrow::Borrow;

/// A fault list stops here, with one `More`: a hostile project must not cost its size in messages (260).
pub const MAX_FAULTS: usize = 1_000;
/// A string a fault holds is cut to this many characters (259).
pub const MAX_HELD: usize = 200;

/// What is wrong, in the TypeScript's own groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultKind {
    ExtendsBroken,
    ExtendsUnknown,
    TimelineCycle,
    TimelineFixed,
    ClipBroken,
    SequenceBroken,
    RefMissing,
    AudioOutside,
    AudioMissing,
    SequenceGain,
    LoopNoTo,
    TimelineBroken,
    StrudelMissing,
    GroupSource,
    TooFew,
    TooMany,
    PortType,
    TapUnrouted,
    ConsoleAbsent,
    More,
}

impl FaultKind {
    pub fn name(self) -> &'static str {
        match self {
            FaultKind::ExtendsBroken => "extends_broken",
            FaultKind::ExtendsUnknown => "extends_unknown",
            FaultKind::TimelineCycle => "timeline_cycle",
            FaultKind::TimelineFixed => "timeline_fixed",
            FaultKind::ClipBroken => "clip_broken",
            FaultKind::SequenceBroken => "sequence_broken",
            FaultKind::RefMissing => "ref_missing",
            FaultKind::AudioOutside => "audio_outside",
            FaultKind::AudioMissing => "audio_missing",
            FaultKind::SequenceGain => "sequence_gain",
            FaultKind::LoopNoTo => "loop_no_to",
            FaultKind::TimelineBroken => "timeline_broken",
            FaultKind::StrudelMissing => "strudel_missing",
            FaultKind::GroupSource => "group_source",
            FaultKind::TooFew => "too_few",
            FaultKind::TooMany => "too_many",
            FaultKind::PortType => "port_type",
            FaultKind::TapUnrouted => "tap_unrouted",
            FaultKind::ConsoleAbsent => "console_absent",
            FaultKind::More => "more",
        }
    }
}

/// Who holds the fault: the place a reader looks for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Holder {
    Track(String),
    Timeline {
        id: String,
        lane: String,
    },
    Module(String),
    /// `step` is 1-based, as the TypeScript prints it.
    Chain {
        name: String,
        step: Option<usize>,
    },
    Node(String),
    File(String),
    Console,
}

/// One fault. The strings it holds are cut to `MAX_HELD` characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    pub kind: FaultKind,
    pub holder: Holder,
    pub target: String,
}

/// `s` cut to `MAX_HELD` characters.
pub fn held(s: &str) -> String {
    s.chars().take(MAX_HELD).collect()
}

impl Holder {
    /// The same holder with every string cut; the original is not consumed, so a caller never clones it whole first (259).
    pub fn cut(&self) -> Holder {
        match self {
            Holder::Track(t) => Holder::Track(held(t)),
            Holder::Timeline { id, lane } => Holder::Timeline {
                id: held(id),
                lane: held(lane),
            },
            Holder::Module(m) => Holder::Module(held(m)),
            Holder::Chain { name, step } => Holder::Chain {
                name: held(name),
                step: *step,
            },
            Holder::Node(n) => Holder::Node(held(n)),
            Holder::File(f) => Holder::File(held(f)),
            Holder::Console => Holder::Console,
        }
    }

    fn spell(&self) -> String {
        match self {
            Holder::Track(t) => format!("track:{t}"),
            Holder::Timeline { id, lane } => format!("timeline:{id}:{lane}"),
            Holder::Module(m) => format!("module:{m}"),
            Holder::Chain { name, step: None } => format!("chain:{name}"),
            Holder::Chain { name, step: Some(n) } => format!("chain:{name}:{n}"),
            Holder::Node(n) => format!("node:{n}"),
            Holder::File(f) => format!("file:{f}"),
            Holder::Console => "console".to_owned(),
        }
    }
}

impl Fault {
    /// A holder by value or by reference; either way only its cut copy is kept.
    pub fn new(kind: FaultKind, holder: impl Borrow<Holder>, target: &str) -> Fault {
        Fault {
            kind,
            holder: holder.borrow().cut(),
            target: held(target),
        }
    }

    /// A chain's fault built from its name as it stands, cut before anything is copied.
    pub fn at_chain(kind: FaultKind, name: &str, step: Option<usize>, target: &str) -> Fault {
        Fault {
            kind,
            holder: Holder::Chain { name: held(name), step },
            target: held(target),
        }
    }

    /// `<kind>|<holder>|<target>`: what a test and the gate compare.
    pub fn key(&self) -> String {
        format!("{}|{}|{}", self.kind.name(), self.holder.spell(), self.target)
    }
}
