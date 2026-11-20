//! bracket.rs — the bracket every native edit of a person's files runs in: the capture window, the journal's step, the account's
//! write lock, the landing, the engine told, the stamp, the step's end. What a reader gets wrong: the gate comes before the lock, never after.

use k7_file_service::changes::commit::Landed;
use k7_file_service::journal::{Ended, Journals, Mode, Owner};

use super::entries::{blocking, told};
use super::journal::{Open, close_window, gate_root, open_window, stamp_after};
use super::{Ctx, locks};
use crate::routes::files::service::routes::Call;

/// How the step opens: its label, its mode, the project whose tree it images.
pub(super) struct Spec<'a> {
    pub label: &'a str,
    pub mode: Mode,
    pub dir: &'a str,
}

/// What the work did under the lock: its own result, what it landed, and the `.k7x` texts it wrote for the stamp.
pub(super) struct Edit<T> {
    pub out: T,
    pub landed: Option<Landed>,
    pub written: Vec<(String, String)>,
}

/// What the bracket came to: the work's result and how the journal's step ended (`None`: it was abandoned).
pub(super) struct Bracketed<T> {
    pub out: T,
    pub ended: Option<Ended>,
}

/// Runs `work` (blocking, with the step's id) under the gate, the lock and the window; `lost` is its result if the task panicked.
/// `None` when the journal would not begin. A step whose work landed nothing is aborted, not ended.
pub(super) async fn run<T, F>(
    c: &Call,
    ctx: &Ctx,
    spec: Spec<'_>,
    lost: T,
    work: F,
) -> Option<Bracketed<T>>
where
    T: Send + 'static,
    F: FnOnce(u64) -> Edit<T> + Send + 'static,
{
    let window = open_window(c, spec.label).await;
    let journals = Journals::global();
    let root = gate_root(ctx);
    // The journal's gate first, the account's lock after it, never the reverse (1733).
    let guard = journals.enter(&root).await;
    let (home, r2, dir, label, mode) = (
        ctx.home.clone(),
        root.clone(),
        spec.dir.to_string(),
        spec.label.to_string(),
        spec.mode,
    );
    let begun = blocking(move || {
        journals.begin_with(guard, &home, &r2, Some(&dir), &label, mode, Owner::mint())
    })
    .await;
    let Some(begun) = begun else {
        close_window(c, window).await;
        return None;
    };
    let _open = Open(begun.id);
    let lock = locks::write(&ctx.home.studio).await;
    let id = begun.id;
    let done = blocking(move || work(id)).await;
    let done = done.unwrap_or(Edit {
        out: lost,
        landed: None,
        written: Vec::new(),
    });
    if done.landed.is_some() {
        told(&c.root, "after").await;
    }
    // The lock goes after `told`, as `PUT /file` drops it: the stamp takes it again for its own write, and the journal's end needs none.
    drop(lock);
    let mut ended = None;
    if done.landed.is_some() {
        for (abs, text) in &done.written {
            stamp_after(c, ctx, &root, abs, text).await;
        }
        ended = blocking(move || journals.end_now(id, None)).await;
    }
    if ended.is_none() {
        journals.abort(id);
    }
    super::journal::count_lossy(ctx);
    close_window(c, window).await;
    Some(Bracketed {
        out: done.out,
        ended,
    })
}
