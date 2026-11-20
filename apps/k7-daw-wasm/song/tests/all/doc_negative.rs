// doc_negative.rs — the document's negative corpus: each case a project with the file sets it must stop on and warn on, and a pointer proving its feature.
// A reader would get wrong that it needs a staged corpus: it needs no TypeScript and runs with a plain `cargo test`.

use crate::doc_support;
use crate::support;

use doc_support::{check, doc_cases, lay, run_disk, shape_failures};
use song::Cache;
use support::{Tmp, case_roots};

#[test]
fn every_case_gives_the_files_and_the_pointers_it_expects_on_disk_and_through_its_bundle() {
    let t = Tmp::new("doc-negative");
    let mut failures = Vec::new();
    let mut cache = Cache::new();
    let cases = doc_cases();
    for case in &cases {
        let base = t.0.join(&case.name);
        lay(&base, case);
        let mut roots = case_roots(&base, None);
        roots.renders = Some(support::spell(&base.join("home/.renders")));
        let (ran, replay) = run_disk(&mut cache, &roots, &case.rig, case.vsts_none, &case.renders, case.mem)
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        failures.extend(check(case, &ran));
        if !ran.engine.is_null() {
            failures.extend(shape_failures(&case.name, &ran.engine).1);
        }
        failures.extend(replay.into_iter().map(|w| format!("{}: {w}", case.name)));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    eprintln!(
        "{} cases, {} with the TypeScript",
        cases.len(),
        cases.iter().filter(|c| c.ts).count()
    );
}
