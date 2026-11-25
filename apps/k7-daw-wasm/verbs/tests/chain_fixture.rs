// chain_fixture.rs — the crate's chain project (`chains/`, `tracks/`) compiles to the lanes and plugins it is written with.
// A reader would get wrong why it is here: e2e's fixtures are all graph projects, and M5's graph verbs edit both layouts.

mod support;

use support::Project;

#[test]
fn the_chain_fixture_compiles_to_its_two_lanes() {
    let mut p = Project::open_crate("chain-lanes");
    let doc = p.doc();
    let ids: Vec<&str> = doc.tracks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, ["drums", "keys"]);
    assert_eq!(p.starts("drums"), [0.0, 1.0]);
    assert_eq!(p.track("drums").plugins.len(), 1, "the compressor plays on the drums");
    assert_eq!(p.track("keys").plugins.len(), 1, "the synth plays on the keys");
}
