//! Where a node's params come from: its own file, or a module it extends, found by the same loader the compile uses.
//! What a reader gets wrong: `own` holds only what the node file states, so a param left to a module is in `from`.

use k7_file_service::libraries::Home;
use k7_file_service::module_reads::{Reading, node_origins};
use serde_json::json;

use crate::support::Scratch;

#[test]
fn a_node_says_what_it_extends_and_which_params_it_sets_itself() {
    let s = Scratch::new("origins");
    s.file(
        "ws/_modules/audio/base/module.k7x",
        "params({ gain: 1, tone: 0.5 })\n",
    );
    s.file(
        "ws/_modules/audio/amp/module.k7x",
        "extends(\"@workspace/audio/base\")\nparams({ tone: 0.7, drive: 3 })\n",
    );
    s.file("ws/song/project.k7x", "name(\"song\")\n");
    s.file(
        "ws/song/nodes/lead.k7x",
        "extends(\"@workspace/audio/amp\")\nparams({ drive: 9 })\n",
    );
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
        admin: false,
    };
    let (root, project) = (s.path("ws"), s.path("ws/song"));
    let code = crate::reads_golden::code();
    let r = Reading {
        home: &home,
        code_root: &code,
        root: &root,
        active: Some(&project),
    };
    let got = node_origins(&r).unwrap_or_else(|_| panic!("origins"));
    assert_eq!(
        got["lead"],
        json!({
            "extends": ["@workspace/audio/amp", "@workspace/audio/base"],
            "own": ["drive"],
            "from": { "tone": "@workspace/audio/amp", "drive": "@workspace/audio/amp", "gain": "@workspace/audio/base" },
        })
    );
}
