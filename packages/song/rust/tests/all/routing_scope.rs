// routing_scope.rs — subchains keyed `<parent>/<id>` and how a link resolves (chainScope.ts; decisions 88, 107).
// A reader would get wrong that a sibling subchain wins over a chain or an end of the same name, but only in its own file.

use crate::support;

use song::routing::{chain_units, split_key, sub_key};
use support::chain_files;

const GTR: &str = "start(true)\nsteps([id(\"lane\").track(\"gtr\"), id(\"amp\").extends(\"x\"), to([chain(\"wet\").level(0.5), chain(\"MIX\").level(1)])])\nsubchains([{id: \"wet\", steps: [id(\"verb\").extends(\"x\"), to([chain(\"MIX\")])]}])\n";
const BASS: &str = "start(true)\nsteps([id(\"b\").extends(\"x\"), to([chain(\"wet\")])])\n";
const WET: &str = "steps([id(\"plate\").extends(\"x\"), to([chain(\"MIX\")])])\n";

#[test]
fn keys_round_trip() {
    assert_eq!(sub_key("gtr", "wet"), "gtr/wet");
    assert_eq!(split_key("gtr/wet"), ("gtr", Some("wet")));
    assert_eq!(split_key("gtr"), ("gtr", None));
}

#[test]
fn a_link_resolves_to_a_sibling_subchain_first_and_to_the_chain_of_that_name_outside() {
    // chainSubchains.test.ts "compiles a link to the subchain inside its parent, and to the chain of that name outside it".
    let chains = chain_files(&[("gtr", GTR), ("bass", BASS), ("wet", WET)]);
    let scope = chain_units(&chains);
    let by = |k: &str| scope.unit(k).expect(k).clone();
    assert_eq!(scope.resolve_link(&by("gtr"), "wet").as_deref(), Some("gtr/wet"));
    assert_eq!(scope.resolve_link(&by("bass"), "wet").as_deref(), Some("wet"));
}

#[test]
fn a_built_in_end_resolves_unless_a_chain_has_that_name() {
    let chains = chain_files(&[("gtr", GTR), ("MIX", WET)]);
    let scope = chain_units(&chains);
    let gtr = scope.unit("gtr").expect("gtr").clone();
    assert_eq!(scope.resolve_link(&gtr, "MIX").as_deref(), Some("MIX"));
    assert_eq!(scope.resolve_link(&gtr, "OUT").as_deref(), Some("OUT"));
    assert!(scope.resolve_link(&gtr, "nowhere").is_none());
}

#[test]
fn a_subchain_reaches_nothing_but_a_sibling_or_an_end() {
    let chains = chain_files(&[("gtr", GTR), ("bass", BASS)]);
    let scope = chain_units(&chains);
    let wet = scope.unit("gtr/wet").expect("wet").clone();
    assert!(scope.resolve_link(&wet, "bass").is_none());
    assert_eq!(scope.resolve_link(&wet, "MIX").as_deref(), Some("MIX"));
}

#[test]
fn units_list_each_files_subchains_right_after_it() {
    let chains = chain_files(&[("gtr", GTR), ("bass", BASS)]);
    let keys: Vec<String> = chain_units(&chains).units.iter().map(|u| u.key.clone()).collect();
    assert_eq!(keys, ["gtr", "gtr/wet", "bass"]);
}
