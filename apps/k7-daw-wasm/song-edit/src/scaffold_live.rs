// scaffold_live.rs — the two `live` archetypes' files: a rig you play THROUGH, an input, a fader and the way out, with no lane.
// A reader would get wrong that `live_project_files` is the GRAPH layout the bench splices into by node name; a new project is written as chain files.

use crate::error::Res;
use crate::fs::Fs;
use crate::ids::NewId;
use crate::scaffold::{files_under, quote, with_ids, write_project_files};

/// The input and the fader it runs into, spelled once since the graph names both.
pub const LIVE_INPUT: &str = "IN";
pub const LIVE_FADER: &str = "fader";
/// The shared strip every mixer node names.
pub const SHARED_STRIP: &str = "@meta/audio/mixer";
/// The lens, owned the way the input is: a built-in io name.
pub const LIVE_CAMERA: &str = "CAM";
const RENDER_NODE: &str = "RENDER";
const VIDEO_OUT_NODE: &str = "VIDEO_OUT";

const DEFAULT_COUNT_IN: u32 = 1;

fn project_text(name: &str, picture: bool) -> String {
    let (lead, about) = if picture {
        (
            " — and SEEN through it.\n",
            "// It declares ONE surface, the viewport, and only because a picture has nowhere else to\n\
// go: an arrangement left undrawn is still on disk, a frame nothing composites is not\n\
// shown at all. No timeline and no desk. See apps/k7-daw-frontend/packages/song/docs/liveProject.md.\n",
        )
    } else {
        (
            ".\n",
            "// It declares no `surfaces/` — no timeline, no viewport, no desk — and that is the whole\n\
// of what makes it this shape rather than another. A project draws what it DECLARES, so\n\
// this one opens on its graph and nothing else. See apps/k7-daw-frontend/packages/song/docs/liveProject.md.\n",
        )
    };
    let frame = if picture {
        "  // The FRAME the lens draws into: [width, height, fps]. A rig has no arrangement, so\n  // this is the only size in the file.\n  viewport: [1280, 720, 30],\n"
    } else {
        ""
    };
    format!(
        "// A LIVE rig: something plugged in, heard through this machine{lead}//\n{about}//\n\
// It holds no material either: no `clips/`, no `sequences/`. Nothing is arranged, so\n\
// nothing warns about an arrangement that is not drawn.\n\
project({{\n  name: {},\n  // Kept although nothing is arranged: the metronome counts in it, and so does a\n  // delay whose time is written in beats.\n  tempo: 120,\n  sampleRate: 48000,\n{frame}}})\n",
        quote(name)
    )
}

fn graph_text(picture: bool) -> String {
    let lens = if picture {
        format!(
            "  // …and the picture's half: the lens composites into the frame, as the fader lands on\n  // the sum. It needs NO video lane — a camera reaching none draws a layer of its own,\n  // the whole frame (apps/k7-daw-frontend/packages/song/docs/camera.md).\n  {LIVE_CAMERA}: [{}],\n  {RENDER_NODE}: [{}],\n",
            quote(RENDER_NODE),
            quote(VIDEO_OUT_NODE)
        )
    } else {
        String::new()
    };
    format!(
        "// The signal graph, as SEGMENTS: each module says where its signal goes next.\n\
//\n\
// There is NO LANE here, and no `nodes/` track file: a run carrying a live input and\n\
// landing on no lane materialises its own track, the way a bus does. So this is the whole\n\
// rig — input, fader, out. See apps/k7-daw-frontend/packages/song/docs/liveProject.md.\n\
//\n\
// Put a module between the two and you have an effects chain, which is what this shape is\n\
// for: `{LIVE_INPUT}: [\"amp\"]`, `amp: [\"{LIVE_FADER}\"]`.\n\
modules({{\n  // The input runs into the fader…\n  {LIVE_INPUT}: [{}],\n  // …and the fader into the mixer's sum, where every run in every project lands. Not\n  // straight to OUT: the engine sums a track into the master either way, so OUT would\n  // DRAW a bypass of the master run below that does not exist.\n  {LIVE_FADER}: [\"MIX\"],\n  // The master run, which every project has: what the bar's record button captures is\n  // what reaches OUT, the whole mix (apps/k7-daw-frontend/packages/song/docs/outputRecording.md).\n  MIX: [\"OUT\"],\n{lens}}})\n",
        quote(LIVE_FADER)
    )
}

const INPUT_NODE: &str = "// The live input off the audio interface — what this project exists to hear.\n\
//\n\
// It names NO channel, and that is deliberate: which socket you are plugged into is a fact\n\
// about your RIG, and no scaffold can guess it. Until you choose one the project says so —\n\
// \"no channel — it feeds nothing\" — which is the one thing worth being told about a rig\n\
// that is not making a sound yet. Pick it on the node's card, or write it here:\n\
//\n\
//     channel([1])        // 1-based; a PAIR for a stereo input, e.g. [1, 2]\n\
//     channel(\"guitar\")   // …or a label from the machine's preferences.yaml (channels.in),\n\
//                         // which is the spelling that survives another machine\n\
//\n\
// `monitor(\"on\")` is what makes it audible without arming anything.\n\
io(\"in\")\n\
monitor(\"on\")\n\
// …and a trim in dB, applied at the interface ahead of everything: 0 is unity.\n\
gain(0)\n";

/// Every file a `live` project is, by project-relative path in the order the TypeScript lists them.
#[must_use]
pub fn live_project_files(name: &str) -> Vec<(String, String)> {
    let fader = format!(
        "// The rig's fader — one USE of the SHARED strip: a level in dB, a pan and three bands,\n\
// which is what a track's level is everywhere in this app. It comes with the face the\n\
// library module carries, so the card draws a fader and four knobs rather than nothing.\n\
//\n\
// A node is where a param belongs, so a second input later gets its own fader without\n\
// either touching the other's level.\n\
extends({})\n\
params({{\n  // Unity, in DECIBELS: -60 is silence and +6 the top. Never 0..1, which would look like\n  // it worked and send half a dB for half a fader.\n  Volume: 0,\n}})\n",
        quote(SHARED_STRIP)
    );
    vec![
        ("project.k7x".to_owned(), project_text(name, false)),
        (
            "settings.k7x".to_owned(),
            format!(
                "// The metronome is OFF: a rig you play through is not a rig you record to a click.\nmetronome(false)\ncount_in({DEFAULT_COUNT_IN})\n"
            ),
        ),
        (format!("nodes/{LIVE_INPUT}.k7x"), INPUT_NODE.to_owned()),
        (format!("nodes/{LIVE_FADER}.k7x"), fader),
        ("graph.k7x".to_owned(), graph_text(false)),
    ]
}

/// Write a new `live` project under `dir` in the graph layout; answers the files' names.
///
/// # Errors
/// A file that does not read, or the file system's.
pub fn scaffold_live_graph(fs: &mut dyn Fs, dir: &str, name: &str) -> Res<Vec<String>> {
    write_project_files(fs, dir, &live_project_files(name))
}

const CAMERA_NODE_TEXT: &str = "// The webcam — what this project exists to SHOW, beside the input it exists to hear.\n\
//\n\
// It names NO lens, for the reason the input names no channel: which camera you are on is\n\
// a fact about your rig. Empty means whichever one this machine calls default, so it draws\n\
// a picture before you have chosen anything. Pick it on the node's card, or write it here:\n\
//\n\
//     camera(\"Logitech BRIO\")   // the LABEL, as the machine reads it out — never a deviceId,\n\
//                               // which is salted per browser and rotates (docs/camera.md)\n\
//     frame([40, 40, 320, 180]) // …and where in the viewport it draws; omit it to fill\n\
io(\"camera\")\n";

const VIEWPORT_SURFACE: &str = "// The FRAME — where the lens is composited with WebGL. The one surface this shape has:\n\
// an arrangement can be left undrawn, but a picture nothing composites is not shown at all.\n\
//\n\
// `floating` is the only layout it draws in: a window over whichever page is on screen,\n\
// moved by its title bar and sized by its corner. `ui viewport` shows or hides it.\n\
type(\"viewport\")\n\
layout(\"floating\")\n";

/// Every file a `live-video` project is: the rig ADDED to, with a lens and the frame it fills.
#[must_use]
pub fn live_video_project_files(name: &str) -> Vec<(String, String)> {
    let mut out = live_project_files(name);
    for (k, t) in &mut out {
        match k.as_str() {
            "project.k7x" => *t = project_text(name, true),
            "graph.k7x" => *t = graph_text(true),
            _ => {}
        }
    }
    out.push((format!("nodes/{LIVE_CAMERA}.k7x"), CAMERA_NODE_TEXT.to_owned()));
    out.push(("surfaces/viewport.k7x".to_owned(), VIEWPORT_SURFACE.to_owned()));
    out
}

/// The chain files of a `live` project: the input and the fader in one run, then the master run; ids 1 to 3 are minted, the first kept by no file.
const LIVE_CHAINS: &[(&str, &str)] = &[
    (
        "chains/IN.k7x",
        r####"start(true)
steps([
  id("<id2>").io("in").monitor("on").gain(0),
  id("<id3>").extends("@meta/audio/mixer").params({ Volume: 0 }),
  to([chain("MIX")]),
])
"####,
    ),
    (
        "chains/master.k7x",
        r####"start("MIX")
steps([to([chain("OUT")])])
"####,
    ),
];

/// The chain files of a `live-video` project: the lens, its picture's run, the rig's own; ids 1 to 5 are minted, the first two kept by no file.
const LIVE_VIDEO_CHAINS: &[(&str, &str)] = &[
    (
        "chains/CAM.k7x",
        r####"start(true)
steps([id("<id5>").io("camera"), to([chain("RENDER")])])
"####,
    ),
    (
        "chains/IN.k7x",
        r####"start(true)
steps([
  id("<id3>").io("in").monitor("on").gain(0),
  id("<id4>").extends("@meta/audio/mixer").params({ Volume: 0 }),
  to([chain("MIX")]),
])
"####,
    ),
    (
        "chains/master.k7x",
        r####"start("MIX")
steps([to([chain("OUT")])])
"####,
    ),
    (
        "chains/picture.k7x",
        r####"start("RENDER")
steps([to([chain("VIDEO_OUT")])])
"####,
    ),
];

/// What the graph layout holds beside the chain files a new project is made of: the node and graph files are left out.
fn kept(files: Vec<(String, String)>) -> Vec<(String, String)> {
    files
        .into_iter()
        .filter(|(k, _)| k != "graph.k7x" && !k.starts_with("nodes/"))
        .collect()
}

/// Write a new `live` project as chain files; answers the files it holds.
///
/// # Errors
/// A file that does not read, or the file system's.
pub fn scaffold_live(fs: &mut dyn Fs, dir: &str, name: &str, new_id: NewId) -> Res<Vec<String>> {
    let mut files = kept(live_project_files(name));
    files.extend(with_ids(LIVE_CHAINS, 3, new_id));
    write_project_files(fs, dir, &files)?;
    Ok(files_under(fs, dir))
}

/// Write a new `live-video` project: the same rig, plus a lens and the frame it draws into.
///
/// # Errors
/// A file that does not read, or the file system's.
pub fn scaffold_live_video(fs: &mut dyn Fs, dir: &str, name: &str, new_id: NewId) -> Res<Vec<String>> {
    let mut files = kept(live_video_project_files(name));
    files.extend(with_ids(LIVE_VIDEO_CHAINS, 5, new_id));
    write_project_files(fs, dir, &files)?;
    Ok(files_under(fs, dir))
}
