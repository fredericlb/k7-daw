//! The bench's guards: nothing is handed back once the rig was rewritten, the rig writes only into its own folder, an admin is declined before any write.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};

use k7_file_service::bench::{BenchError, Libs, ensure_bench};
use k7_file_service::edits::DiskFs;
use k7_file_service::libraries::Home;
use song::session::Session;
use song_edit::Fs;

use crate::support::Scratch;

const CHAIN: &str = "steps([\n  id(\"11111111-1111-4111-8111-111111111111\").extends(\"@k7-core/audio/verb\"),\n  to([chain(\"@out\")]),\n])\n";

fn world(name: &str) -> (Scratch, Home) {
    let s = Scratch::new(name);
    s.file(
        "libraries/k7-core/modules/audio/verb/module.k7x",
        "extends(\"@engine/reverb\")\n",
    );
    s.file("libraries/k7-core/chains/solo.k7x", CHAIN);
    s.file("home/workspace/Banc/_library-pins.k7x", "k7-core(1)\n");
    let home = Home {
        studio: s.path("home"),
        libraries: s.path("libraries"),
        admin: false,
    };
    (s, home)
}

fn open(home: &Home, project: &str, vocabulary_reads: bool) -> (Result<(), BenchError>, bool) {
    let env: BTreeMap<String, String> = BTreeMap::new();
    let code = format!("{}/../../../../..", env!("CARGO_MANIFEST_DIR"));
    let vocabulary = move |roots: &song::Roots, disk: &song::DiskSource| {
        if vocabulary_reads {
            Session::new(roots.paths).vocabulary(disk, roots).ok()
        } else {
            None
        }
    };
    let libs = Libs {
        home,
        code_root: &code,
        env: &env,
        vocabulary: &vocabulary,
    };
    let wrote = AtomicBool::new(false);
    let done = ensure_bench(&libs, project, &wrote);
    (done, wrote.load(Ordering::SeqCst))
}

// A chain that declines after the rig was rewritten is refused, not handed back, and the next open makes the rig again.
#[test]
fn a_decline_after_the_rewrite_is_a_refusal_and_the_next_open_splices() {
    let (s, home) = world("eb-decline");
    let chain = "@k7-core/solo/live.k7";
    assert!(open(&home, chain, true).0.is_ok());
    let graph = s.path("home/workspace/Banc/live.k7/graph.k7x");
    assert!(std::fs::read_to_string(&graph).unwrap().contains("verb"));
    let (done, wrote) = open(&home, chain, false);
    assert!(matches!(done, Err(BenchError::Refused(_))), "{done:?}");
    assert!(wrote);
    assert!(!std::path::Path::new(&s.path("home/workspace/Banc/bench-source.json")).exists());
    assert!(open(&home, chain, true).0.is_ok());
    assert!(std::fs::read_to_string(&graph).unwrap().contains("verb"));
}

// An admin is declined before anything is written.
#[test]
fn an_admin_is_declined_and_nothing_is_written() {
    let (s, mut home) = world("eb-admin");
    home.admin = true;
    let (done, wrote) = open(&home, "@k7-core/solo/live.k7", true);
    assert!(matches!(done, Err(BenchError::Declined(_))), "{done:?}");
    assert!(!wrote);
    assert!(!std::path::Path::new(&s.path("home/workspace/Banc/live.k7")).exists());
}

// The rig's disk reads the libraries and writes only into the bench's folder.
#[test]
fn the_rig_writes_only_into_its_own_folder() {
    let (s, _) = world("eb-writes");
    let (bench, lib) = (s.dir("home/workspace/Banc/live.k7"), s.path("libraries"));
    let mut fs = DiskFs::reading(&[bench.as_str()], &[lib.as_str()]).unwrap();
    fs.mkdir_all(&format!("{bench}/nodes")).unwrap();
    fs.write_atomic(&format!("{bench}/nodes/x.k7x"), b"x")
        .unwrap();
    assert!(
        fs.write_atomic(&format!("{lib}/k7-core/x.k7x"), b"x")
            .is_err()
    );
    assert!(fs.mkdir_all(&format!("{lib}/new")).is_err());
    assert!(
        fs.remove(&format!("{lib}/k7-core/chains/solo.k7x"))
            .is_err()
    );
}
