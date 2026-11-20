//! fsx.rs — the batched write and the durability pass: a replaced file is whole, a missing one is skipped, a pass names what failed.
//! What a reader gets wrong: nothing here can show a power loss; it shows the contract a pass leaves on a live disk.

use std::path::PathBuf;

use k7_sync::fsx::{make_durable, write_batched};

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Dir {
        let dir = std::env::temp_dir().join(format!("k7-sync-fsx-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Dir(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn a_batched_write_creates_and_replaces_whole_and_leaves_no_temp() {
    let dir = Dir::new("batched");
    let file = dir.0.join("a.md");
    write_batched(&file, "one", false).unwrap();
    write_batched(&file, "two, longer", false).unwrap();
    write_batched(&file, "three", true).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "three");
    let names: Vec<_> = std::fs::read_dir(&dir.0).unwrap().collect();
    assert_eq!(names.len(), 1, "a temp was left behind");
}

#[test]
fn a_pass_syncs_every_file_skips_a_missing_one_and_works_on_any_thread_count() {
    let dir = Dir::new("pass");
    let files: Vec<PathBuf> = (0..40).map(|i| dir.0.join(format!("f{i}.md"))).collect();
    for f in files.iter().take(39) {
        write_batched(f, "text", false).unwrap();
    }
    for threads in [1, 4, 64] {
        make_durable(&files, threads).unwrap();
    }
    make_durable(&[], 4).unwrap();
}

#[test]
#[ignore = "a measurement: cargo test --release -p k7-sync -- --ignored --nocapture"]
fn measure_a_durability_pass_serial_and_on_four_threads() {
    for files in [3_135, 12_540] {
        for threads in [1, 4, 8] {
            let dir = Dir::new("measure");
            let paths: Vec<PathBuf> = (0..files)
                .map(|i| {
                    let d = dir.0.join(format!("d{}", i / 100));
                    std::fs::create_dir_all(&d).unwrap();
                    d.join(format!("f{i}.md"))
                })
                .collect();
            let t0 = std::time::Instant::now();
            for p in &paths {
                write_batched(p, "text of a file\n", false).unwrap();
            }
            let wrote = t0.elapsed();
            let t0 = std::time::Instant::now();
            make_durable(&paths, threads).unwrap();
            println!(
                "{files} files: written unsynced in {wrote:?}; durability pass on {threads} thread(s): {:?}",
                t0.elapsed()
            );
        }
    }
}
