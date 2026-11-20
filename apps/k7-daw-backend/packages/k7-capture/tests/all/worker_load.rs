//! worker_load.rs — the shared k7x settler under several busy workspaces at once: measured and printed, bounded only by a backstop.
//! What a reader gets wrong: the figure is for the brief's decision about whole texts (2106), not an assertion about speed.

use std::time::{Duration, Instant};

use crate::rig::put;
use crate::worker::Bed;

const ACCOUNTS: usize = 4;
const FILES: usize = 300;

fn k7x(n: usize, pan: usize) -> String {
    format!("plugin(\"mixer\")\n// strip {n}\nparams({{ gain: 0.5, pan: {pan} }})\n")
}

/// Ops across every batch a bed has emitted: a burst may close as more than one.
fn total(b: &Bed) -> usize {
    b.batches()
        .iter()
        .map(|x| x["ops"].as_array().unwrap().len())
        .sum()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_shared_settler_over_four_accounts_each_changing_three_hundred_k7x_files_is_measured() {
    let mut beds = vec![];
    for a in 0..ACCOUNTS {
        beds.push(Bed::new(&format!("load-{a}"), false, None).await);
    }
    for round in 0..2 {
        let t0 = Instant::now();
        for b in &beds {
            for n in 0..FILES {
                let rel = format!("s{}.k7/nodes/n{n}.k7x", n % 10);
                put(&b.root, &rel, k7x(n, round));
                b.fake.report(&[&rel]);
            }
        }
        for b in &beds {
            b.until("every file in a batch", |b| total(b) >= (round + 1) * FILES)
                .await;
        }
        let kinds: Vec<String> = beds[0]
            .batches()
            .iter()
            .flat_map(|b| b["ops"].as_array().unwrap().clone())
            .skip(round * FILES)
            .take(FILES)
            .collect::<Vec<_>>()
            .iter()
            .map(|o| o["kind"].as_str().unwrap().to_string())
            .collect();
        let settled = kinds.iter().filter(|k| *k == "k7x").count();
        println!(
            "round {round}: {ACCOUNTS} accounts x {FILES} k7x files in {:?}; account 0: {settled} k7x ops, {} whole texts",
            t0.elapsed(),
            kinds.len() - settled
        );
        assert!(t0.elapsed() < Duration::from_secs(60));
    }
}
