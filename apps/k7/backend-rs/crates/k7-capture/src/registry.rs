//! registry.rs — the process's captures: one `Account` per studio home, found again by a workspace's root, and the one
//! native k7x half every account's capture and the relay go through (`Captures`, 2105). What a reader gets wrong: the
//! window calls are synchronous and answer false for a root nobody captures, which is how a caller learns there is no capture.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use k7_sync::settle::Native;

use crate::account::{Account, Attached, Host};

/// The process's k7x half: stateless, so the relay's and every account's capture's share one.
pub fn native() -> Arc<Native> {
    static ONE: OnceLock<Arc<Native>> = OnceLock::new();
    ONE.get_or_init(|| Arc::new(Native)).clone()
}

/// The accounts that capture, by studio home.
#[derive(Default)]
pub struct Captures {
    accounts: Mutex<HashMap<String, Arc<Account>>>,
    /// One gate per home: a start and a stop of the same home run one after the other.
    gates: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Captures {
    pub fn global() -> &'static Captures {
        static ALL: OnceLock<Captures> = OnceLock::new();
        ALL.get_or_init(Captures::default)
    }

    /// Starts the account of `host.home`, attaching every workspace on `rt`; one already running is stopped first (2105).
    pub async fn start_account(
        &self,
        log: Arc<dyn Fn(&str) + Send + Sync>,
        host: Host,
        rt: &tokio::runtime::Handle,
    ) -> Arc<Account> {
        let home = host.home.clone();
        let gate = self.gate(&home);
        let _one = gate.lock().await;
        let old = lock(&self.accounts).remove(&home);
        if let Some(old) = old {
            old.stop_all().await;
        }
        let acct = Arc::new(Account::new(log));
        acct.bind(host);
        lock(&self.accounts).insert(home, acct.clone());
        let a = acct.clone();
        rt.spawn(async move { a.attach_all().await });
        acct
    }

    fn gate(&self, home: &str) -> Arc<tokio::sync::Mutex<()>> {
        lock(&self.gates)
            .entry(home.to_string())
            .or_default()
            .clone()
    }

    /// Stops the account now registered for `home`, each ledger saved, and forgets it.
    pub async fn stop_account(&self, home: &str) {
        if let Some(a) = self.account(home) {
            self.stop_this(home, &a).await;
        }
    }

    /// Stops `acct` only while it is still the one registered: a start that replaced it has stopped it already.
    pub async fn stop_this(&self, home: &str, acct: &Arc<Account>) {
        let gate = self.gate(home);
        let _one = gate.lock().await;
        let mine = {
            let mut all = lock(&self.accounts);
            let same = all.get(home).is_some_and(|c| Arc::ptr_eq(c, acct));
            if same { all.remove(home) } else { None }
        };
        if let Some(a) = mine {
            a.stop_all().await;
        }
    }

    pub fn account(&self, home: &str) -> Option<Arc<Account>> {
        lock(&self.accounts).get(home).cloned()
    }

    /// The capture of the workspace at `root`, and the account holding it.
    pub fn at(&self, root: &str) -> Option<(Arc<Account>, Arc<Attached>)> {
        let all: Vec<Arc<Account>> = lock(&self.accounts).values().cloned().collect();
        all.into_iter()
            .find_map(|a| a.attached_at(root).map(|at| (a, at)))
    }

    /// Opens a mutation window on `root`'s capture; false when nothing captures it (2121).
    pub fn hold(&self, root: &str) -> bool {
        self.at(root).map(|(_, a)| a.handle.hold()).is_some()
    }

    /// Closes a window; the last one's release sweeps. False when nothing captures `root`.
    pub fn release(&self, root: &str, label: &str) -> bool {
        self.at(root)
            .map(|(_, a)| a.handle.release(label))
            .is_some()
    }
}
