//! settler_scale.rs — a measurement run by hand (`-- --ignored --nocapture`, in release): N accounts joining a library at
//! once, each diffing every k7x file of `k7-core` through the process's one native k7x half (2106).
//! What a reader gets wrong: nothing here asserts a time; the figures go to `file-capture.md`.

use std::time::Instant;

use k7_capture::native;
use k7_replica::ops::K7x;

fn k7x_files(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            k7x_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "k7x")
            && let Ok(text) = std::fs::read_to_string(&p)
        {
            out.push((p.display().to_string(), text));
        }
    }
}

/// Each of `n` threads settles every file once; answers each thread's wall time in milliseconds, sorted.
fn burst(
    pool: &std::sync::Arc<k7_sync::settle::Native>,
    files: &[(String, String)],
    n: usize,
) -> Vec<u128> {
    let mut walls: Vec<u128> = std::thread::scope(|s| {
        let joins: Vec<_> = (0..n)
            .map(|_| {
                s.spawn(|| {
                    let t0 = Instant::now();
                    for (f, text) in files {
                        let _ = pool.k7x_op_for(f, text, &text.replacen('0', "1", 1));
                    }
                    t0.elapsed().as_millis()
                })
            })
            .collect();
        joins.into_iter().map(|j| j.join().unwrap()).collect()
    });
    walls.sort_unstable();
    walls
}

#[test]
#[ignore = "a measurement: cargo test --release -- --ignored --nocapture"]
fn a_burst_of_library_joins_through_the_process_k7x_half() {
    let mut files = vec![];
    k7x_files(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../../k7-core"),
        &mut files,
    );
    println!(
        "{} k7x files per join, {} cores",
        files.len(),
        std::thread::available_parallelism().unwrap()
    );
    let pool = native();
    for n in [1, 4, 13, 32] {
        let w = burst(&pool, &files, n);
        println!(
            "{n:>2} joins: fastest {} ms, median {} ms, slowest {} ms",
            w[0],
            w[n / 2],
            w[n - 1]
        );
    }
}
