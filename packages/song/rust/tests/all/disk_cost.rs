// disk_cost.rs — what the disk Source's real-path checks cost over a tree of 2,000 folders (decision 761).
// A reader would get wrong that the numbers are asserted tightly: they are printed, and only a generous bound holds, since Windows is measured by CI.

use crate::support;

use song::{DiskSource, Source};
use std::time::{Duration, Instant};
use support::{Tmp, spell};

fn hang_bound() -> Duration {
    Duration::from_secs(if cfg!(target_env = "musl") { 60 } else { 15 })
}

// 761: one compile's worth of verdicts, the Source's stat of each of 2,000 folders and then of each again, against plain stats.
#[test]
fn the_real_path_checks_over_two_thousand_folders_are_measured_and_bounded() {
    let t = Tmp::new("disk-cost");
    let paths: Vec<String> = (0..2_000)
        .map(|i| {
            t.write(&format!("d{i}/f.txt"), "x");
            t.at(&format!("d{i}"))
        })
        .collect();
    let plain = Instant::now();
    for p in &paths {
        assert!(std::fs::symlink_metadata(p).is_ok());
    }
    let plain = plain.elapsed();
    let src = DiskSource::new(&[spell(&t.0)]).expect("a source");
    let first = Instant::now();
    for p in &paths {
        assert!(src.stat(p).is_ok(), "{p}");
    }
    let first = first.elapsed();
    let again = Instant::now();
    for p in &paths {
        assert!(src.stat(p).is_ok(), "{p}");
    }
    let again = again.elapsed();
    eprintln!(
        "disk cost over 2,000 folders: plain lstat {plain:?}, first pass of the Source {first:?}, the same again {again:?}"
    );
    assert!(first < hang_bound() && again < hang_bound(), "{first:?} {again:?}");
}
