// playlists.rs — the workspace's set lists: new, song add/remove/move, and stems separated by the front's route.
// A reader would get wrong where a playlist is: `_playlists/` at the workspace root, a file outside the project folder.

mod support;

use serde_json::json;
use support::{Project, WS};

fn items(p: &Project) -> String {
    p.fs.text(&format!("{WS}/_playlists/gig.k7x")).unwrap_or_default()
}

#[test]
fn a_playlist_is_made_filled_and_reordered() {
    let mut p = Project::open("mute-solo").with_core();
    let file = format!("{WS}/_playlists/gig.k7x");
    let ran = p.round_trip("playlist new gig");
    assert_eq!(ran.changed(&p.dir), [file.as_str()]);
    p.ok("playlist new gig");
    assert_eq!(items(&p), "// gig\ntitle(\"gig\")\nitems([])\n");
    p.refused("playlist new gig", "_playlists/gig.k7x is already there");
    p.refused("playlist new ../out", "would leave _playlists/");
    p.refused("playlist new set.yaml", "a playlist is a .k7x file");

    let ran = p.round_trip("playlist song add gig Hells Bells");
    assert_eq!(ran.answer.log, ["_playlists/gig.k7x: + Hells Bells — 1 song"]);
    p.ok("playlist song add gig Hells Bells");
    p.ok("playlist song add gig \"Back in Black\" live");
    p.ok("playlist song add gig Thunder");
    assert!(
        items(&p).contains("song(\"Back in Black\").variant(\"live\")"),
        "{}",
        items(&p)
    );

    let ran = p.round_trip("playlist song move gig 3 1");
    assert_eq!(ran.answer.log, ["_playlists/gig.k7x: Thunder → 1/3"]);
    p.ok("playlist song move gig 3 1");
    let t = items(&p);
    assert!(t.find("Thunder") < t.find("Hells Bells"), "{t}");
    let ran = p.ok("playlist song remove gig 2");
    assert_eq!(ran.answer.log, ["_playlists/gig.k7x: − Hells Bells"]);
    p.refused(
        "playlist song remove gig 9",
        "has 2 songs — name a position between 1 and 2",
    );
    p.refused("playlist song move gig 1", "both positions go from 1 to 2");
    p.refused("playlist song add ghost x", "no playlist _playlists/ghost.k7x");
    p.refused("playlist song add gig", "usage: playlist song add");

    let ran = p.ok_answered("playlist stems gig @k7-core/stems", |c| {
        assert_eq!(c["route"], "POST /verb-host/playlist-stems");
        assert_eq!(
            c["args"],
            json!({ "songs": ["Thunder", "Back in Black"], "steps": [{
                "step": "@k7-core/cold/stemsplit",
                "model": "/home-none/models/htdemucs_fp16weights.onnx",
                "outputs": ["drums", "bass", "other", "vocals"],
            }] })
        );
        json!({ "job": "j1", "dir": "/jobs/j1", "songs": [
            { "song": "Thunder", "state": "done", "files": [{ "out": "a" }, { "out": "b" }] },
            { "song": "Back in Black", "state": "failed", "error": "the model hung\nat 12%" },
        ] })
    });
    assert_eq!(
        ran.answer.log,
        [
            "_playlists/gig.k7x through '@k7-core/stems': 1 separated",
            "Thunder: 2 stem(s)",
            "Back in Black: the model hung"
        ]
    );
    assert_eq!(ran.before, ran.after);
    p.refused("playlist stems gig @k7-core/process/normalize", "does not separate");
    p.refused("playlist stems gig", "no separating chain");
    p.ok("playlist new empty");
    p.refused("playlist stems empty @k7-core/stems", "names no song");
}
