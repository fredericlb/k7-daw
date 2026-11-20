//! stack.rs — a blocking job on a thread of its own with the compiler's stack, answering through a oneshot.
//! What a reader gets wrong: tokio's blocking pool has 2 MiB stacks and an overflow is not a panic, it aborts
//! the whole front; every native compile runs here (docs/file-docs.md, 1522).

use std::panic::{AssertUnwindSafe, catch_unwind};

use tokio::sync::oneshot;

/// The stack of a compile's thread: a deep project's recursion needs far more than tokio's 2 MiB.
pub const THREAD_STACK: usize = 64 << 20;

/// Why a job gave no answer.
pub enum Failed {
    /// The job panicked.
    Panicked,
    /// The thread could not be made or went away without answering.
    Lost,
}

/// `f` on a fresh thread with `THREAD_STACK`, awaited.
pub async fn on_stack<R, F>(f: F) -> Result<R, Failed>
where
    R: Send + 'static,
    F: FnOnce() -> R + Send + 'static,
{
    let (tx, rx) = oneshot::channel();
    let spawned = std::thread::Builder::new()
        .name("k7-front compile".into())
        .stack_size(THREAD_STACK)
        .spawn(move || {
            let _ = tx.send(catch_unwind(AssertUnwindSafe(f)));
        });
    if spawned.is_err() {
        return Err(Failed::Lost);
    }
    match rx.await {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(_)) => Err(Failed::Panicked),
        Err(_) => Err(Failed::Lost),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Each level holds 64 KiB, so 100 levels need 6 MiB: past tokio's blocking pool, inside the compiler's stack.
    fn descend(n: usize) -> usize {
        let buf = [0u8; 64 << 10];
        std::hint::black_box(&buf);
        if n == 0 { 0 } else { descend(n - 1) + 1 }
    }

    #[tokio::test]
    async fn a_job_gets_the_compilers_stack_not_the_blocking_pools() {
        assert_eq!(on_stack(|| descend(100)).await.ok(), Some(100));
    }

    #[tokio::test]
    async fn a_panic_comes_back_as_a_value_and_the_next_job_runs() {
        let r = on_stack(|| -> u8 { panic!("a compile that dies") }).await;
        assert!(matches!(r, Err(Failed::Panicked)));
        assert_eq!(on_stack(|| 7).await.ok(), Some(7));
    }
}
