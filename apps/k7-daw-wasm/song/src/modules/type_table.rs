// type_table.rs — the type folders' default ports and the io roles: the one source of these tables.
// A reader would get wrong that the page holds its own copy: k7-wasm's bindings binary writes them into consts.generated.ts.

use super::types::TypeMeta;
use super::{ModuleKind, PortType};
use PortType::{Audio, Midi, Video};

/// The type folders, in the order a scan walks them.
pub const MODULE_TYPES: [TypeMeta; 13] = [
    TypeMeta {
        name: "io",
        kind: ModuleKind::Io,
        inputs: &[],
        outputs: &[],
    },
    // A note on the graph: one input, no output, and its `audio` type is only the ordinary cable (song-schema's docs/moduleTypes.md).
    TypeMeta {
        name: "docs",
        kind: ModuleKind::Doc,
        inputs: &[("in", Audio, Some(false))],
        outputs: &[],
    },
    TypeMeta {
        name: "instruments",
        kind: ModuleKind::Plugin,
        inputs: &[("midi", Midi, None)],
        outputs: &[("out", Audio, None)],
    },
    TypeMeta {
        name: "audio",
        kind: ModuleKind::Plugin,
        inputs: &[("in", Audio, None)],
        outputs: &[("out", Audio, None)],
    },
    TypeMeta {
        name: "transformers",
        kind: ModuleKind::Plugin,
        inputs: &[("midi", Midi, None)],
        outputs: &[("out", Midi, None)],
    },
    TypeMeta {
        name: "video_effects",
        kind: ModuleKind::Plugin,
        inputs: &[("in", Video, None)],
        outputs: &[("out", Video, None)],
    },
    // Named by a placement, never wired: ports stay declared (empty means unconstrained) and the graph's validation refuses the cable.
    TypeMeta {
        name: "video_transitions",
        kind: ModuleKind::Plugin,
        inputs: &[("in", Video, None)],
        outputs: &[("out", Video, None)],
    },
    // Picture in, picture out, and an audio side input allowed to be empty (apps/k7-daw-wasm/verbs/docs/picture/shaders.md).
    TypeMeta {
        name: "shaders",
        kind: ModuleKind::Plugin,
        inputs: &[("in", Video, None), ("audio", Audio, Some(false))],
        outputs: &[("out", Video, None)],
    },
    // A picture source on a lane like a lens; `inputs: []` is unconstrained, and the graph's validation refuses a cable in.
    TypeMeta {
        name: "shapes",
        kind: ModuleKind::Plugin,
        inputs: &[],
        outputs: &[("out", Video, None)],
    },
    TypeMeta {
        name: "midi",
        kind: ModuleKind::Plugin,
        inputs: &[("midi", Midi, None)],
        outputs: &[("out", Midi, None)],
    },
    // Audio in, audio out, run over a whole file before playback (apps/k7-daw-wasm/song/docs/coldModules.md).
    TypeMeta {
        name: "cold",
        kind: ModuleKind::Plugin,
        inputs: &[("in", Audio, None)],
        outputs: &[("out", Audio, None)],
    },
    // One half of a summing console: placed by the compile from `project.console`, never wired by hand (apps/k7-daw-wasm/song/docs/console.md).
    TypeMeta {
        name: "console",
        kind: ModuleKind::Plugin,
        inputs: &[("in", Audio, None)],
        outputs: &[("out", Audio, None)],
    },
    // A lane: two outputs since it is not typed by what it carries, and no input on purpose.
    TypeMeta {
        name: "tracks",
        kind: ModuleKind::Track,
        inputs: &[],
        outputs: &[("audio", Audio, None), ("midi", Midi, None)],
    },
];

/// The io roles the app provides: node name and its `io`.
pub const IO_MODULES: [(&str, &str); 6] = [
    ("IN", "in"),
    ("OUT", "out"),
    // A live picture, and the one io source that emits video: the browser opens it, not the engine.
    ("CAM", "camera"),
    // The headphone bus: an `out` like the interface, a second pair of it.
    ("CUE", "out"),
    // Audio off disk rather than off an interface; a node carries which file.
    ("FILE", "file"),
    // A MIDI keyboard: the notes a lane records, from the device its node names.
    ("MIDI", "midi"),
];

/// The io roles the signal comes FROM; every other io is a sink.
pub const IO_SOURCES: [&str; 4] = ["in", "file", "camera", "midi"];
