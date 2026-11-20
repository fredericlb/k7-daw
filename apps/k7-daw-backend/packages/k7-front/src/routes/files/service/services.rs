//! services.rs — the accounts the front serves: each made on its first request, with the environment a file
//! service is given and a secret of its own, which the relay and the hub's internal routes take as a service's.
//! What a reader gets wrong: nothing runs per account but its native pieces; no process is started for one.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use k7_hub::Log;
use k7_hub::account::{Account, Names, random_secret};
use serde_json::json;
use subtle::ConstantTimeEq;

use super::accounts::{self, Account as Acct};
use super::native;
use super::switch::code_root;

/// What every account hears when the front has no code root: node is never started in the front's place.
const NO_CODE_ROOT: &str = "file service: no code root holds the Rust file service's files (K7_CODE_ROOT, or a checkout above the front)";

/// What the libraries answer when their runtime could not be made.
const NO_LIBRARIES: &str = "libraries: the front's libraries were not made (its log says why)";

/// Set in an account's environment where the front captures its workspaces and holds its relay link.
pub const FRONT_CAPTURE: &str = "K7_FRONT_CAPTURE";

/// Whether the front captures for an account of environment `env`.
pub(super) fn captures(env: &BTreeMap<String, String>) -> bool {
    env.get(FRONT_CAPTURE).map(String::as_str) == Some("1")
}

/// An account the front serves: its identity, its pieces, and what stops with it.
pub struct Held {
    pub account: Account,
    /// The native account: the song sessions and the environment the pieces read (A1).
    pub acct: Arc<Acct>,
    /// The front's log, which the account's pieces say their lines to.
    pub log: Log,
    /// Where `packages/` and `demos/` are read.
    pub code_root: PathBuf,
    /// The account's ring, registry and machine asks, where `runtime` answers (2606: none otherwise).
    pub runtime: Option<Arc<native::runtime::Runtime>>,
    /// What the pieces stop with the account, in order (2671).
    pub members: native::runtime::stop::Members,
    /// The native capture runs for this account: a hidden server's workspaces are then the capture's to name (2650).
    pub native_capture: bool,
    /// The account's waveforms, downloads and takes (1l).
    pub media: Option<Arc<native::media::Media>>,
    /// The account's engine link (2210): it lives and dies with the account.
    pub engine: Option<Arc<native::engine::routes::Engine>>,
}

impl Held {
    /// The account's lines for the Logs panel and the front's log alike (`serviceLine`).
    pub fn say(&self, level: &str, line: &str) {
        (self.log)(line);
        if let Some(r) = &self.runtime {
            r.say(level, line);
        }
    }
}

/// The accounts the front serves and what they are made from.
pub struct Services {
    /// The song sessions' heap in bytes, the server's or an embedded host's.
    heap: usize,
    /// What a file service may see of the process and the host's own settings, which every account's environment starts from.
    base: BTreeMap<String, String>,
    /// Where `packages/` and `demos/` are read; without one no account is taken.
    pub(super) code_root: Option<PathBuf>,
    /// By user.
    held: Mutex<HashMap<String, Arc<Held>>>,
    /// One account made at a time, so two first requests of an account make one.
    pub(super) making: tokio::sync::Mutex<()>,
    /// Each account's own turn at being made: its environment, and the publish token minted in it, made once.
    turns: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    /// The workspaces each account's capture was told a request named (`join`), so it is told once.
    pub(super) joined: Mutex<HashSet<(String, String)>>,
    pub(super) log: Log,
    /// Set once the stop began: no account is made after it (2671).
    stopping: AtomicBool,
}

impl Services {
    pub fn new(env: &BTreeMap<String, String>, log: Log) -> Services {
        let code_root = code_root(env);
        match &code_root {
            Some(r) => log(&format!(
                "the Rust file service answers every account's files, code at {}",
                r.display()
            )),
            None => log(
                "no code root holds the Rust file service's files (K7_CODE_ROOT, or a checkout): every account's files are refused",
            ),
        }
        Services {
            heap: accounts::sessions_heap(),
            base: accounts::base_env(env),
            code_root,
            held: Mutex::default(),
            making: tokio::sync::Mutex::new(()),
            turns: Mutex::default(),
            joined: Mutex::default(),
            log,
            stopping: AtomicBool::new(false),
        }
    }

    /// Where `packages/` and `demos/` are read, when one holds the file service's files.
    pub(super) fn code_root(&self) -> Option<&std::path::Path> {
        self.code_root.as_deref()
    }

    /// Why `user`'s files are refused: no code root holds the file service's files.
    pub fn refusal(&self, _user: &str) -> Option<&'static str> {
        self.code_root.is_none().then_some(NO_CODE_ROOT)
    }

    /// Why the libraries are refused when the front holds no runtime of them: node never runs them.
    pub fn libraries_refusal(&self) -> &'static str {
        self.refusal("").unwrap_or(NO_LIBRARIES)
    }

    /// Whether the origin's libraries are the front's own: the file service's files are here.
    pub fn libraries_in_front(&self) -> bool {
        self.code_root.is_some()
    }

    /// Whether the front takes `user`'s routes: every account, where a code root is there.
    pub fn covers(&self, _user: &str) -> bool {
        self.code_root.is_some()
    }

    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<Held>>> {
        self.held.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// How many accounts are held now.
    pub fn running(&self) -> usize {
        self.map().len()
    }

    /// The account while it is held, never made for asking.
    pub fn get(&self, user: &str) -> Option<Arc<Held>> {
        self.map().get(user).cloned()
    }

    /// The account, made with the environment and a secret of its own if it is not held.
    pub async fn held(&self, user: &str, names: &Arc<Names>) -> Result<Arc<Held>, String> {
        if let Some(h) = self.get(user) {
            return Ok(h);
        }
        if self.stopping.load(Ordering::Acquire) {
            return Err("stopping".to_string());
        }
        let code_root = self.code_root.clone().ok_or("no code root")?;
        // A second first request waits here: minting the environment again would revoke the token the first one holds.
        let turn = {
            let mut turns = self.turns.lock().unwrap_or_else(|p| p.into_inner());
            turns.retain(|_, t| Arc::strong_count(t) > 1);
            turns.entry(user.to_string()).or_default().clone()
        };
        let _turn = turn.lock().await;
        if let Some(h) = self.get(user) {
            return Ok(h);
        }
        let account = accounts::identity(names, &self.base, user, &random_secret()?).await?;
        let acct = Acct::make(account.env.clone(), self.heap);
        // The capture is native for an account the front captures for (2111).
        let native_capture = captures(&account.env);
        // Examples is stocked before `making`, under the account's own lock, so one slow mirror stalls no other account (1642).
        if native_capture {
            native::workspaces_start::stocked(&account.env, &code_root, &self.log).await;
        }
        let _one = self.making.lock().await;
        if let Some(h) = self.get(user) {
            return Ok(h);
        }
        let media = Some(Arc::new(native::media::Media::new(
            native::media::Config::of(&account.env, &code_root),
            Arc::new(|| ()),
        )));
        let engine = Some(native::engine::routes::Engine::start(
            &account,
            &acct,
            user,
            &code_root,
            self.log.clone(),
        ));
        // The account's ring, registry and machine asks, over the engine link (2606).
        let runtime = engine.as_ref().map(|e| {
            native::runtime::Runtime::with_watching(
                Arc::new(native::runtime::link_machine::LinkMachine(e.link.clone())),
                !account.env.contains_key("K7_NO_FS_WATCH"),
            )
        });
        if let (Some(e), Some(r)) = (&engine, &runtime) {
            let _ = e.runtime.set(r.clone());
        }
        // The prefetch needs the link's binding and sessions, so it starts where both exist (2460).
        if let (Some(m), Some(e)) = (&media, &engine) {
            let (weak, say) = (Arc::downgrade(m), self.log.clone());
            let moved: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                if let Some(m) = weak.upgrade() {
                    m.say_if_due(&say);
                }
            });
            m.prefetch.start(
                Arc::new(native::media::prefetch::LinkSource(e.link.clone())),
                m.counts.clone(),
                moved,
                native::media::prefetch::FIRST,
                native::media::prefetch::EVERY,
            );
        }
        if runtime.is_some() {
            native::runtime::report::start_once(&self.log, native::runtime::report::REPORT_EVERY);
        }
        (self.log)(&format!("[{}] account up", account.slug));
        if native_capture {
            native::capture_start::start(
                engine.clone(),
                &account.env,
                &account.slug,
                &self.log,
                runtime.as_ref().map(|r| r.watch.clone()),
            )
            .await;
        }
        let h = Arc::new(Held {
            account,
            acct,
            log: self.log.clone(),
            code_root,
            runtime,
            members: native::runtime::stop::Members::default(),
            native_capture,
            media,
            engine,
        });
        // The flag is read again under the map's lock, which `stop` takes after setting it (2671).
        let mut map = self.map();
        if self.stopping.load(Ordering::Acquire) {
            drop(map);
            // The account's whole stop, its capture's ledger saved (2671).
            let _ = h.stop(None, None).await;
            return Err("stopping".to_string());
        }
        map.insert(user.to_string(), h.clone());
        Ok(h)
    }

    /// The account's binding, told to its engine link before a request reaches it (docs/service.md).
    pub async fn bind(
        &self,
        held: &Held,
        bound: Option<&k7_hub::binding::Bound>,
        seam: &k7_hub::seam::Seam,
    ) {
        let binding = match bound {
            Some(b) => k7_hub::binding::told_json(seam, &*b.state.lock().await),
            None => json!({ "url": null }),
        };
        if let Some(e) = &held.engine {
            e.link.bind(&binding);
        }
    }

    /// The account a secret proves, compared in constant time.
    pub fn by_secret(&self, secret: &str) -> Option<Account> {
        if secret.is_empty() {
            return None;
        }
        self.map()
            .values()
            .find(|h| bool::from(h.account.secret.as_bytes().ct_eq(secret.as_bytes())))
            .map(|h| h.account.clone())
    }

    /// Every account stopped at once and awaited, bounded in all; what had not stopped is named in one line (2670).
    pub async fn stop(&self) {
        self.stopping.store(true, Ordering::Release);
        let helds: Vec<Arc<Held>> = self.map().drain().map(|(_, h)| h).collect();
        if let Some(line) = native::runtime::stop::stop_all(helds, native::runtime::STOP).await {
            (self.log)(&line);
        }
    }
}

#[cfg(test)]
impl Held {
    /// A rig's account over `acct`: no runtime, media or members.
    pub(crate) fn of_rig(
        account: Account,
        acct: Arc<Acct>,
        code_root: PathBuf,
        engine: Option<Arc<native::engine::routes::Engine>>,
        native_capture: bool,
    ) -> Held {
        Held {
            account,
            acct,
            log: Arc::new(|_: &str| {}),
            code_root,
            runtime: None,
            members: native::runtime::stop::Members::default(),
            native_capture,
            media: None,
            engine,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn an_account_is_captured_only_where_the_hub_says_so() {
        let mut env = BTreeMap::from([(FRONT_CAPTURE.to_string(), "1".to_string())]);
        assert!(captures(&env));
        env.insert(FRONT_CAPTURE.to_string(), "0".to_string());
        assert!(!captures(&env));
        assert!(!captures(&BTreeMap::new()));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_accounts_mirror_never_stalls_another_account() {
        let dir = std::env::temp_dir().join(format!("k7-accounts-mirror-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let demos = dir.join("demos/synth.k7");
        std::fs::create_dir_all(&demos).unwrap();
        std::fs::write(demos.join("project.k7x"), "project({})\n").unwrap();
        let data = dir.join("data");
        let mut o = k7_hub::account::Names::new(Arc::new(move |u: &str| {
            let slug = u.replace('@', "-");
            (slug.clone(), data.join(slug))
        }));
        let demos = dir.join("demos").display().to_string();
        o.env = Arc::new(move |_, user| {
            let demos = demos.clone();
            Box::pin(async move {
                let capture = if user == "a@x.org" { "1" } else { "0" };
                BTreeMap::from([
                    (FRONT_CAPTURE.to_string(), capture.to_string()),
                    ("STUDIO_DEMOS".to_string(), demos),
                ])
            })
        });
        let instances = Arc::new(o);
        let env = BTreeMap::new();
        let svc = Arc::new(Services::new(&env, Arc::new(|_: &str| {})));
        assert!(svc.code_root.is_some());
        let a = instances.environment("a@x.org", "s").await.unwrap();
        // A's account lock is held, so A's mirror waits behind it.
        let held_a =
            native::locks::write(&k7_file_service::libraries::Home::of(&a.env).studio).await;
        let (svc2, inst2) = (svc.clone(), instances.clone());
        let waiting = tokio::spawn(async move { svc2.held("a@x.org", &inst2).await.map(|_| ()) });
        tokio::time::sleep(Duration::from_millis(300)).await;
        let b =
            tokio::time::timeout(Duration::from_secs(20), svc.held("b@x.org", &instances)).await;
        assert!(b.is_ok_and(|r| r.is_ok()), "B waited behind A's mirror");
        assert!(!waiting.is_finished());
        drop(held_a);
        assert!(waiting.await.unwrap().is_ok());
        svc.stop().await;
        let _ = std::fs::remove_dir_all(dir);
    }
}
