//! k7x — a .k7x file's change as values set or deleted at key paths, and the replay of them, native (`k7xOps.ts`).
//! The diff, the locate rule, the replay and the settle share one tree reader and the document editor of `packages/k7x`.
//! What a reader gets wrong: the TypeScript's answers are vectors (`fixtures/settle-vectors.json`) are the referee.

mod apply;
mod diff;
mod locate;

use serde_json::{Value, json};

pub use apply::{apply_k7x, apply_within, settle_k7x};
pub use diff::{Diff, diff_k7x};
pub use locate::{
    SegCache, Segs, Work, fragment, gone_at, item_segments, list_keys, locate, value_at,
};

/// What one pass of `k7x_op_for` may spend: bytes reparsed and list items keyed; past it the change is a whole text.
const OP_WORK: u64 = 40_000_000;

/// The `kind: "k7x"` op for a change to a k7x file, or None when only a whole text replays to `after` (`fileOpFor`).
pub fn k7x_op_for(file: &str, before: &str, after: &str) -> Option<Value> {
    // Item ops first; when they do not replay, the same diff with every list one value.
    for lists in [true, false] {
        let work = Work::limited(OP_WORK);
        let diff = diff_k7x(before, after, lists);
        let keyed = diff.as_ref().is_some_and(|d| d.keyed);
        if let Some(d) = diff
            && !d.ops.is_empty()
            && apply_within(before, &d.ops, &work).is_ok_and(|t| t == after)
        {
            return Some(json!({ "kind": "k7x", "file": file, "ops": d.ops }));
        }
        if !keyed {
            break;
        }
    }
    None
}
