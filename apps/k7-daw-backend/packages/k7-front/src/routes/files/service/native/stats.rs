//! stats.rs — the native pieces' counts for the whole process, in atomics, and the decline said once (1009, 1010).
//! What a reader gets wrong: the line keeps the `[native-shadow] compared` prefix the bed's judge reads, whatever it counts.

use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use k7_hub::Log;

use super::{Ctx, Piece};

/// What was said once per workspace and reason, kept to this many before it starts over (1010).
const SAID_CAP: usize = 1024;

/// The counts of a run, for the whole process, in atomics (1009).
#[derive(Default)]
pub struct Stats {
    pub declined: AtomicU64,
    /// What `docs` answered, a compile failure included.
    pub docs_answered: AtomicU64,
    /// What `docs` declined (a compile it could not make), apart from `declined`.
    pub docs_declined: AtomicU64,
    /// What `chains` declined, apart from `declined`: the spec's unreadable chain declines on purpose (1p-D19).
    pub chains_declined: AtomicU64,
    /// Work refused for want of capacity, a busy decline (1780a).
    pub shed: AtomicU64,
    /// The part of `shed` that came with fewer than `BURST` requests in flight: a shed no burst explains (1780a).
    pub shed_lone: AtomicU64,
    pub io: AtomicU64,
    pub panics: AtomicU64,
    /// Journal reads that repaired invalid UTF-8 or a lone surrogate, once per read of the file (1796).
    pub lossy: AtomicU64,
    said: Mutex<HashSet<(String, &'static str)>>,
}

/// The counts of this process.
pub fn global() -> &'static Stats {
    static GLOBAL: OnceLock<Stats> = OnceLock::new();
    GLOBAL.get_or_init(Stats::default)
}

impl Stats {
    pub(super) fn bump(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }

    /// One shed, lone where fewer than `BURST` were in flight (1780a).
    pub(super) fn shed(&self, in_flight: usize) {
        Self::bump(&self.shed);
        if in_flight < super::answering::BURST {
            Self::bump(&self.shed_lone);
        }
    }

    /// A decline said once per workspace and reason (1010).
    pub(super) fn say_declined(&self, ctx: &Ctx, why: &'static str) {
        super::refuse::note(why);
        let subject = format!("{}/{}", ctx.account, ctx.workspace);
        let route = ctx.tag.route();
        if why == "busy" {
            self.shed(ctx.gate.in_flight(&ctx.account));
        } else if ctx.tag.piece() == Piece::Docs {
            Self::bump(&self.docs_declined);
        } else if ctx.tag.piece() == Piece::Chains {
            Self::bump(&self.chains_declined);
        } else {
            Self::bump(&self.declined);
        }
        self.say_once(&subject, route, &ctx.workspace, &ctx.log, why);
    }

    /// The same for a route that names no one workspace: `subject` keys what was said, `what` words the line.
    pub(super) fn declined_for(
        &self,
        subject: &str,
        route: &str,
        what: &str,
        log: &Log,
        why: &'static str,
    ) {
        super::refuse::note(why);
        Self::bump(&self.declined);
        self.say_once(subject, route, what, log, why);
    }

    fn say_once(&self, subject: &str, route: &str, what: &str, log: &Log, why: &'static str) {
        let mut said = self.said.lock().unwrap_or_else(|p| p.into_inner());
        if said.len() >= SAID_CAP {
            said.clear();
        }
        if said.insert((subject.to_string(), why)) {
            log(&format!(
                "[files] native: {route} declined for '{what}' ({why}), refused"
            ));
        }
    }

    /// The report line (1009).
    pub fn line(&self) -> String {
        let get = |c: &AtomicU64| c.load(Ordering::Relaxed);
        format!(
            "[native-shadow] compared declined={} docsAnswered={} docsDeclined={} shed={} shedlone={} io={} panics={} lossy={} chainsDeclined={}",
            get(&self.declined),
            get(&self.docs_answered),
            get(&self.docs_declined),
            get(&self.shed),
            get(&self.shed_lone),
            get(&self.io),
            get(&self.panics),
            get(&self.lossy),
            get(&self.chains_declined)
        )
    }
}
