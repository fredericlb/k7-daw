// lanes_memory.rs — what the ownership map holds is one entry per chain and per step, never a copy of a chain's name per step (decision 283).
// A reader would get wrong that a time bound is enough: the copies are made once each, so the peak of live memory is what is measured.

#[path = "all/lanes_support/mod.rs"]
mod lanes_support;
#[path = "all/support/mod.rs"]
mod support;

use lanes_support::inputs;
use song::lanes::ownership::chain_ownership;
use song::routing::{Chain, Out, UseStep};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is forwarded to the system allocator unchanged; only counters beside it are touched.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

// 283: one chain with a name of 512 KB and 6,000 distinct steps holds no copy of the name per step (3 GB when it did).
#[test]
fn a_long_chain_name_is_not_copied_per_step() {
    let mut i = inputs(&[("nodes/mic.k7x", "extends(\"@meta/track\")\n")]);
    let steps: Vec<UseStep> = (0..6_000)
        .map(|n| UseStep {
            module: format!("s{n}"),
            inputs: None,
            enabled: None,
        })
        .collect();
    let name = "n".repeat(512 * 1024);
    let long = Chain {
        from: Some("mic".to_owned()),
        from_port: None,
        use_steps: steps,
        out: vec![Out {
            to: "OUT".to_owned(),
            gain: 1.0,
            from_port: None,
        }],
    };
    i.chains.insert(name.clone(), long);
    let lanes = i.lanes();
    let before = LIVE.load(Ordering::Relaxed);
    PEAK.store(before, Ordering::Relaxed);
    let own = chain_ownership(&i, &lanes);
    let grown = PEAK.load(Ordering::Relaxed) - before;
    assert_eq!(own.side_input_track("s5999"), Some(name.as_str()));
    // The name is held a few times (one per map); 6,000 more copies would be 3 GB.
    assert!(grown < 16 * 1024 * 1024, "the pass grew live memory by {grown} bytes");
}
