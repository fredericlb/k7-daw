// scaffold_files.rs — the text of a new arrangement or video project's files, as the file service lays them down: chain files, one lane file per lane.
// A reader would get wrong the `<idN>` marks: they are the lane and node ids a scaffold mints, numbered in the order the goldens hold.

/// The files the arrangement and the video archetype share, project-relative; `project.k7x` is built in `scaffold.rs`.
pub const ARRANGEMENT: &[(&str, &str)] = &[
    (
        "settings.k7x",
        r####"metronome(false)
count_in(1)
"####,
    ),
    (
        "tracks/melody.k7x",
        r####"// The melody lane. `@meta/track` holds events written down here — each one naming a
// sequence or a clip, and leaving by the lane's midi or audio output accordingly.
extends("@meta/track")
events([ref("melody").start("1:1:1")])
"####,
    ),
    (
        "tracks/live.k7x",
        r####"// A group lane: no material and no signal of its own, just members.
extends("@meta/group-track")
tracks(["pad", "kit"])
"####,
    ),
    (
        "tracks/pad.k7x",
        r####"// A GENERATED lane: its clips are streamed from the pattern while the transport rolls,
// which is why it says `strudel` where an authored lane says `events`.
extends("@meta/strudel-track")
strudel("algo/pattern.strudel")
"####,
    ),
    (
        "tracks/kit.k7x",
        r####"// The other half of the same pattern. Nothing here says "audio": an event whose sound
// resolves to a sample plays as one, every other event plays as a note.
extends("@meta/strudel-track")
strudel("algo/pattern.strudel")
"####,
    ),
    (
        "sequences/melody.k7x",
        r####"notes([
  t(0).d(1).k(60),
  t(1).d(1).k(62),
  t(2).d(1).k(64),
  t(3).d(1).k(65),
  t(4).d(1).k(67),
  t(5).d(1).k(69),
  t(6).d(1).k(71),
  t(7).d(1).k(72),
])
"####,
    ),
    (
        "algo/pattern.strudel",
        r####"// A live Strudel (TidalCycles-in-JS) pattern, linked by the "pad" and "kit"
// tracks. While the transport plays it streams onto them, locked to the beat —
// edit + save to hear it change live.
//
// `.label(...)` names the lane a part plays on. A label matching a strudel
// lane's name lands there — "Pad" on the "pad" lane, so it plays through that
// lane's chain and mixer strip. A label no lane declares still plays, on a
// live track named after it; so does an unlabelled part, bucketed by orbit.
//
// What `s(...)` means depends on WHICH lane module the lane extends: a waveform
// on "pad" (a sequence lane — its chain brings the instrument), a sample name on
// "kit" (an audio lane) — the bundled kit, your project `samples/` folder, or
// $DIRT_SAMPLES.
stack(
  note("c e g c5").s("sawtooth").slow(2).label("Pad"),
  sound("bd hh sn hh").label("Kit"),
)
"####,
    ),
    (
        "curves/.gitignore",
        r####"# Keeps curves/ in the repository while it is still empty.
# Intentionally ignores nothing — a pattern here would hide the curves themselves.
"####,
    ),
    (
        "surfaces/timeline.k7x",
        r####"// The ARRANGEMENT — the timeline, and the transport that rolls through it.
//
// Delete this file and the song still compiles and still plays; you simply have no lanes
// on screen. `split` puts it in the layout, sharing the window with the graph beneath —
// the only layout a timeline draws in. See apps/k7-daw-wasm/song/docs/surfaces.md.
type("timeline")
layout("split")
"####,
    ),
];

/// The arrangement's chain files: lanes `melody`, `pad`, `kit`, `live`, the synths of the first two, and the master run; ids 1 to 7 are minted, the first one kept by no file.
pub const ARRANGEMENT_CHAINS: &[(&str, &str)] = &[
    (
        "chains/kit.k7x",
        r####"start(true)
steps([id("<id4>").track("kit")])
"####,
    ),
    (
        "chains/live.k7x",
        r####"start(true)
steps([id("<id5>").track("live")])
"####,
    ),
    (
        "chains/master.k7x",
        r####"start("MIX")
steps([to([chain("OUT")])])
"####,
    ),
    (
        "chains/melody.k7x",
        r####"start(true)
steps([
  id("<id2>").track("melody"),
  id("<id6>").extends("@workspace/instruments/synth"),
  to([chain("MIX")]),
])
"####,
    ),
    (
        "chains/pad.k7x",
        r####"start(true)
steps([
  id("<id3>").track("pad"),
  id("<id7>").extends("@workspace/instruments/synth"),
  to([chain("MIX")]),
])
"####,
    ),
];

/// What the video archetype adds beside the arrangement's shared files.
pub const VIDEO_EXTRA: &[(&str, &str)] = &[
    (
        "surfaces/viewport.k7x",
        r####"// The FRAME — the video lanes composited with WebGL, drawn against the playhead.
//
// `floating` is the only layout it draws in: a window over whichever page is on screen,
// moved by its title bar and sized by its corner. `ui viewport` shows or hides it; hidden,
// it keeps its GL context and its <video> per layer, so showing it again costs nothing.
//
// Delete this file and the lanes still compile and still draw on the timeline — nothing
// composites them. See apps/k7-daw-wasm/song/docs/surfaces.md.
type("viewport")
layout("floating")
"####,
    ),
    (
        "clips/sample.k7x",
        r####"// A SOURCE, not a placement: this says which file, and a picture lane says where it goes.
// One clip file can be placed any number of times, on any number of lanes.
source("clips/sample.mp4")
"####,
    ),
    (
        "tracks/main.k7x",
        r####"// A lane of picture. Every time here is DECIMAL SECONDS — picture is cut against the
// wall clock, where the lanes above are placed in bars against the tempo. Both are drawn
// on the one timeline, whose ruler reads either (the clock button in its header).
//
// Re-typing `tempo` therefore slides the GRID under this shot rather than moving it:
// the file says seconds, and seconds is what it keeps saying.
//
// `length(0)` runs to the end of the source; `from` trims the head. Muting it
// HIDES it — there is no sound on a picture lane.
extends("@meta/picture-track")
frame([0, 0, 1280, 720])
events([ref("sample").start(0).length(6).from(0)])
"####,
    ),
];

/// The video archetype's chain files: the arrangement's, one id later, with the `main` lane and the picture's run; ids 1 to 9 are minted, the first two kept by no file.
pub const VIDEO_CHAINS: &[(&str, &str)] = &[
    (
        "chains/kit.k7x",
        r####"start(true)
steps([id("<id5>").track("kit")])
"####,
    ),
    (
        "chains/live.k7x",
        r####"start(true)
steps([id("<id6>").track("live")])
"####,
    ),
    (
        "chains/main.k7x",
        r####"start(true)
steps([id("<id7>").track("main")])
"####,
    ),
    (
        "chains/master.k7x",
        r####"start("MIX")
steps([to([chain("OUT").level(1), chain("RENDER").port("audio").level(1)])])
"####,
    ),
    (
        "chains/melody.k7x",
        r####"start(true)
steps([
  id("<id3>").track("melody"),
  id("<id8>").extends("@workspace/instruments/synth"),
  to([chain("MIX")]),
])
"####,
    ),
    (
        "chains/pad.k7x",
        r####"start(true)
steps([
  id("<id4>").track("pad"),
  id("<id9>").extends("@workspace/instruments/synth"),
  to([chain("MIX")]),
])
"####,
    ),
    (
        "chains/picture.k7x",
        r####"start("RENDER")
steps([to([chain("VIDEO_OUT")])])
"####,
    ),
];
