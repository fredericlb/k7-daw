//! tunnel part — one agent per registry the engine is listed at, the way IN for a server that cannot dial this
//! machine: it follows the settings' list (and `$STUDIO_REGISTRY_URL`), starts an agent for a registry that has
//! none and stops the one of a registry that left. What a reader gets wrong: the agents dial whether or not the
//! engine is up, and the jobs they answer are the table's (`table.rs`), where each job is one line.

mod files;
mod guard;
mod log;
mod render;
mod separate;
mod settings;
mod slot_jobs;
mod table;

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::time::{MissedTickBehavior, interval};

use crate::core::{Ctx, Event, PartSpec};
use crate::tunnel::{Options, TunnelAgent};

/// How often the undo audio servers asked this machine to keep is looked over; the first look is at the start.
const PURGE_EVERY: std::time::Duration = std::time::Duration::from_secs(3600);

/// The undo audio kept a month past its last use goes, off the part's task.
fn purge_kept_history_now(cx: &Ctx) {
    let root = cx.paths.studio_dir().join("remote");
    tokio::task::spawn_blocking(move || {
        let n = crate::machine::history::purge_kept_history(
            &root,
            crate::machine::history::KEPT_HISTORY_TTL,
        );
        if n > 0 {
            crate::log::line(
                "supervisor",
                &format!("purged {n} kept undo file(s) older than a month"),
            );
        }
    });
}

pub fn spec() -> PartSpec {
    PartSpec {
        name: "tunnel",
        commands: &[],
        start,
    }
}

fn start(cx: Ctx, rx: mpsc::UnboundedReceiver<Event>) {
    tokio::spawn(run(cx, rx));
}

/// The options of one registry's agent: this machine's identity, the engine's port as it then is, its jobs.
pub fn options(cx: &Ctx, registry: &str, rendering: &Arc<render::Rendering>) -> Options {
    let mut o = Options::new(
        registry,
        &cx.identity.id,
        &cx.identity.token,
        cx.paths.remote_cache(&crate::urls::cache_key(registry)),
    );
    o.tunnel_secret = cx.identity.tunnel_secret.clone();
    let (engine, settings) = (cx.engine.clone(), cx.settings.clone());
    o.engine_port = Arc::new(move || match engine.borrow().port {
        0 => settings.borrow().port,
        port => port,
    });
    o.jobs = table::jobs(cx, rendering);
    o.log = Arc::new(|m| crate::log::line("tunnel", m));
    o
}

/// An agent for every registry that has none, and none for a registry that left the list.
fn follow(
    cx: &Ctx,
    rendering: &Arc<render::Rendering>,
    agents: &mut BTreeMap<String, TunnelAgent>,
) {
    let wanted = crate::registry::resolve(&cx.settings.borrow().registry_urls);
    agents.retain(|u, a| {
        let keep = wanted.contains(u);
        if !keep {
            a.stop();
        }
        keep
    });
    for u in wanted {
        agents
            .entry(u.clone())
            .or_insert_with(|| TunnelAgent::start(options(cx, &u, rendering)));
    }
}

async fn run(cx: Ctx, mut rx: mpsc::UnboundedReceiver<Event>) {
    let mut agents: BTreeMap<String, TunnelAgent> = BTreeMap::new();
    let rendering = render::Rendering::new(&cx);
    let mut purge = interval(PURGE_EVERY);
    purge.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut settings = cx.settings.clone();
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                None => return,
                Some(Event::Boot(ack)) => {
                    follow(&cx, &rendering, &mut agents);
                    let _ = ack.send(());
                }
                Some(Event::Shutdown(ack)) => {
                    agents.values().for_each(TunnelAgent::stop);
                    agents.clear();
                    let _ = ack.send(());
                }
                Some(Event::EngineDown { ack: Some(ack), .. }) => {
                    let _ = ack.send(());
                }
                Some(_) => {}
            },
            Ok(()) = settings.changed() => follow(&cx, &rendering, &mut agents),
            _ = purge.tick() => purge_kept_history_now(&cx),
        }
    }
}
