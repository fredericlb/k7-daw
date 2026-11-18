// scaffold_files.rs — the text of a new project's files, as the TypeScript's `projectFiles` writes them.
// A reader would get wrong that this is copied text: the golden replay of `scaffold` is what keeps it equal to the TypeScript's.

/// The arrangement's static files, project-relative; `project.k7x` and the video graph are built in `scaffold.rs`.
pub const ARRANGEMENT: &[(&str, &str)] = &[
    (
        "settings.k7x",
        r####"metronome(false)
count_in(1)
"####,
    ),
    (
        "nodes/synth-melody.k7x",
        r####"// "synth-melody" — the melody lane's own 4OSC. Add `params` to change this one
// alone; everything else comes from the module it extends.
extends("@workspace/instruments/synth")
"####,
    ),
    (
        "nodes/synth-pad.k7x",
        r####"// "synth-pad" — the pattern's Pad lane gets its own 4OSC, so tweaking one lane's
// sound leaves the other's alone.
extends("@workspace/instruments/synth")
"####,
    ),
    (
        "graph.k7x",
        r####"// The signal graph, as SEGMENTS: each module says where its signal goes next. The
// chains are DERIVED by following the arrows — a run breaks where the signal splits,
// and where two signals meet (which is what a bus is).
//
// The LANES are here too ("melody", "pad"): a lane is a module like any other, and a
// run is named after the lane that starts it. One map, one name space — which is why
// the synths are "synth-melody" and "synth-pad" rather than sharing their lanes' names.
//
// A module is named once here, so one name means one plugin: pointing both lanes at
// one synth would sum them into a single instrument instead.
//
// `bypassed([ … ])` lists the modules the signal passes through untouched — and on a
// lane, which produces rather than passes, that IS its mute.
//
// Two built-in outputs: `MIX` is the mixer's sum, where every lane ends, and `OUT` is
// the interface — which carries the song's output level as its `Volume` (the fader
// under the header meter). The run between them is the MASTER CHAIN: put a module on
// this line and it processes the whole mix on its way out.
modules({
  melody: ["synth-melody"],
  pad: ["synth-pad"],
  synth-melody: ["MIX"],
  synth-pad: ["MIX"],
  MIX: ["OUT"],
})
"####,
    ),
    (
        "nodes/melody.k7x",
        r####"// The melody lane. `@meta/track` holds events written down here — each one naming a
// sequence or a clip, and leaving by the lane's midi or audio output accordingly.
extends("@meta/track")
events([ref("melody").start("1:1:1")])
"####,
    ),
    (
        "nodes/live.k7x",
        r####"// A group lane: no material and no signal of its own, just members.
extends("@meta/group-track")
tracks(["pad", "kit"])
"####,
    ),
    (
        "nodes/pad.k7x",
        r####"// A GENERATED lane: its clips are streamed from the pattern while the transport rolls,
// which is why it says `strudel` where an authored lane says `events`.
extends("@meta/strudel-track")
strudel("algo/pattern.strudel")
"####,
    ),
    (
        "nodes/kit.k7x",
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
// the only layout a timeline draws in. See packages/song/docs/surfaces.md.
type("timeline")
layout("split")
"####,
    ),
];

/// What the video archetype adds beside the arrangement's files.
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
// composites them. See packages/song/docs/surfaces.md.
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
        "nodes/main.k7x",
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
