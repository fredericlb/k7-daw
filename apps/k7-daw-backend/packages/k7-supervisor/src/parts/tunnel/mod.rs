//! tunnel part — one agent per registry the engine is listed at, the way IN for a server that cannot dial this
//! machine: it follows the settings' list (and `$STUDIO_REGISTRY_URL`), starts an agent for a registry that has
//! none and stops the one of a registry that left. What a reader gets wrong: the agents dial whether or not the
//! engine is up, and the jobs they answer are the table's (`table.rs`), where each job is one line.

mod guard;
mod separate;
mod table;

use std::collections::BTreeMap;
use std::sync::Arc;

use tokio::sync::mpsc;

use crate::core::{Ctx, Event, PartSpec};
use crate::tunnel::{Options, TunnelAgent};

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
pub fn options(cx: &Ctx, registry: &str) -> Options {
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
    o.jobs = table::jobs(cx);
    o.log = Arc::new(|m| crate::log::line("tunnel", m));
    o
}

/// An agent for every registry that has none, and none for a registry that left the list.
fn follow(cx: &Ctx, agents: &mut BTreeMap<String, TunnelAgent>) {
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
            .or_insert_with(|| TunnelAgent::start(options(cx, &u)));
    }
}

async fn run(cx: Ctx, mut rx: mpsc::UnboundedReceiver<Event>) {
    let mut agents: BTreeMap<String, TunnelAgent> = BTreeMap::new();
    let mut settings = cx.settings.clone();
    loop {
        tokio::select! {
            ev = rx.recv() => match ev {
                None => return,
                Some(Event::Boot(ack)) => {
                    follow(&cx, &mut agents);
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
            Ok(()) = settings.changed() => follow(&cx, &mut agents),
        }
    }
}
