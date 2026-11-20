//! The scan wait (2222) against the TypeScript's traces: a link's frames and a turned clock in, the holds, the resends and
//! the poll's view out. The 60 s stall and the 10 minute bound are driven on the clock, not waited for.

use crate::support::engine::{rows, vectors};

#[test]
fn the_ten_minute_bound_is_a_count_not_a_wait() {
    let cases = vectors()["scan"].as_array().unwrap();
    let case = cases
        .iter()
        .find(|c| c["name"].as_str().unwrap().contains("ten minutes"))
        .unwrap();
    let want = rows(&case["rows"]);
    assert!(
        want.iter()
            .any(|(t, w)| w == "resend" && (600_000..=601_000).contains(t)),
        "{want:?}"
    );
}
