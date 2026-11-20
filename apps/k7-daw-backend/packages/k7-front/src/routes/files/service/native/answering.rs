//! answering.rs — what answering mode may hold at once: a weight for the process and a share of it per account.
//! What a reader gets wrong: the weight is asked for twice, a base up front and the rest once a file's size is known.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// The memory every native answer in flight may hold, in bytes of weight (docs/file-native.md, 1057).
pub const PROCESS_WEIGHT: usize = 512 << 20;
/// What one account may hold of it, so one account cannot take the process's.
pub const ACCOUNT_SHARE: usize = 128 << 20;
/// What a file or a media path weighs before its size is known.
pub const BASE_WEIGHT: usize = 4 << 20;
/// What a save weighs before its own bytes and its base's are added (1780a).
pub const PUT_BASE: usize = 64 << 10;
/// Requests of one account in flight at once from which a shed is part of a burst (1780a).
pub const BURST: usize = 8;
/// What a tree weighs: its text is bounded by `max_bytes` (64 MiB) and so are its names, with room for the response.
pub const TREE_WEIGHT: usize = 72 << 20;
/// What a host member's call weighs: its work is a job or a probe, the bodies are small (3f).
pub const CALL_WEIGHT: usize = 4 << 20;
/// How long a read waits for its weight before it declines busy.
pub const WAIT: Duration = Duration::from_secs(2);
/// Accounts remembered before the table starts over.
const ACCOUNTS_CAP: usize = 1024;

/// One account's share of weight, and how many of its requests are waiting for it or holding it.
struct Account {
    share: Arc<Semaphore>,
    flight: AtomicUsize,
}

/// One request in flight for an account, counted until dropped.
struct Flight(Arc<Account>);

impl Flight {
    fn new(a: &Arc<Account>) -> Flight {
        a.flight.fetch_add(1, Ordering::Relaxed);
        Flight(a.clone())
    }
}

impl Drop for Flight {
    fn drop(&mut self) {
        self.0.flight.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The weights of a process.
pub struct Gate {
    process: Arc<Semaphore>,
    share: usize,
    wait: Duration,
    accounts: Mutex<HashMap<String, Arc<Account>>>,
}

/// Weight taken from the process and from the account, given back together when dropped.
pub struct Held {
    process: OwnedSemaphorePermit,
    account: OwnedSemaphorePermit,
    // The semaphores the permits came from: a table that started over must not hand `grow` another.
    sems: (Arc<Semaphore>, Arc<Semaphore>),
    _flight: Flight,
}

impl Gate {
    pub fn new(process: usize, share: usize) -> Gate {
        Gate {
            process: Arc::new(Semaphore::new(process)),
            share,
            wait: WAIT,
            accounts: Mutex::default(),
        }
    }

    /// The same, waiting `wait` for weight.
    #[cfg(test)]
    pub fn waiting(mut self, wait: Duration) -> Gate {
        self.wait = wait;
        self
    }

    fn account(&self, name: &str) -> Arc<Account> {
        let mut accounts = self.accounts.lock().unwrap_or_else(|p| p.into_inner());
        if accounts.len() >= ACCOUNTS_CAP && !accounts.contains_key(name) {
            accounts.clear();
        }
        accounts
            .entry(name.to_string())
            .or_insert_with(|| {
                Arc::new(Account {
                    share: Arc::new(Semaphore::new(self.share)),
                    flight: AtomicUsize::new(0),
                })
            })
            .clone()
    }

    /// The requests of `account` waiting for weight or holding it (1780a).
    pub fn in_flight(&self, account: &str) -> usize {
        self.account(account).flight.load(Ordering::Relaxed)
    }

    /// `weight` for `account`, waiting up to `WAIT`; `None` where there is none to be had.
    pub async fn take(&self, account: &str, weight: usize) -> Option<Held> {
        let w = u32::try_from(weight).ok()?;
        let acct = self.account(account);
        let share = acct.share.clone();
        let flight = Flight::new(&acct);
        let both = async {
            let a = share.clone().acquire_many_owned(w).await.ok()?;
            let p = self.process.clone().acquire_many_owned(w).await.ok()?;
            Some(Held {
                process: p,
                account: a,
                sems: (self.process.clone(), share.clone()),
                _flight: flight,
            })
        };
        tokio::time::timeout(self.wait, both).await.ok().flatten()
    }

    /// The weight free in the process, for a test.
    #[cfg(test)]
    pub fn free(&self) -> usize {
        self.process.available_permits()
    }
}

impl Held {
    /// `grow`, retried until `WAIT` is spent: a share full of a tree's weight frees within moments.
    pub async fn grow_waiting(&mut self, more: usize) -> bool {
        let end = tokio::time::Instant::now() + WAIT;
        loop {
            if self.grow(more) {
                return true;
            }
            if tokio::time::Instant::now() >= end {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }

    /// `more` weight on top, without waiting; false (and nothing taken) where either side lacks it.
    pub fn grow(&mut self, more: usize) -> bool {
        let Ok(w) = u32::try_from(more) else {
            return false;
        };
        let Ok(a) = self.sems.1.clone().try_acquire_many_owned(w) else {
            return false;
        };
        let Ok(p) = self.sems.0.clone().try_acquire_many_owned(w) else {
            return false;
        };
        self.account.merge(a);
        self.process.merge(p);
        true
    }
}

/// The gate of this process.
pub fn global() -> &'static Gate {
    static GLOBAL: OnceLock<Gate> = OnceLock::new();
    GLOBAL.get_or_init(|| Gate::new(PROCESS_WEIGHT, ACCOUNT_SHARE))
}
