//! k7-engine — engine trust for k7's Rust backend: the registry and its two stores, the relay and its gate,
//! the tunnel hub. HTTP is the front's (`k7-front`'s `routes/engine`); this crate decides who is let in and
//! moves the frames. Its choices and its security review: `apps/k7/backend-rs/docs/engine.md`.

pub mod clock;
pub mod frame;
pub mod js;
pub mod log;
pub mod registry;
pub mod relay;
pub mod tunnel;
pub mod ws;

use std::sync::Arc;

pub use clock::Clock;
pub use registry::Registry;
pub use registry::ticket::Tickets;
pub use tunnel::hub::Hub;

/// The engine core one front holds: the registry, the hub over it, and the process's ticket secret.
pub struct Core {
    pub registry: Registry,
    pub hub: Arc<Hub>,
    pub tickets: Tickets,
    pub clock: Clock,
    /// The testbed's stand-in for `verify`, as the TypeScript suites inject one.
    #[cfg(feature = "test-hooks")]
    pub verify_stub: std::sync::Mutex<Option<tunnel::hub::Verify>>,
}

/// What the core is built from.
pub struct CoreOptions {
    pub registry: registry::Options,
    pub render_key: Option<String>,
    pub ping_ms: u64,
    pub min_protocol: u32,
    pub uploads: tunnel::uploads::UploadLimits,
    pub registry_log: log::Sink,
    pub tunnel_log: log::Sink,
}

impl Default for CoreOptions {
    fn default() -> Self {
        CoreOptions {
            registry: registry::Options::default(),
            render_key: None,
            ping_ms: 20_000,
            min_protocol: 1,
            uploads: Default::default(),
            registry_log: log::registry_sink(),
            tunnel_log: log::tunnel_sink(),
        }
    }
}

impl Core {
    pub fn new(opts: CoreOptions, clock: Clock) -> Arc<Self> {
        Arc::new_cyclic(|me: &std::sync::Weak<Core>| {
            let registry = Registry::new(opts.registry, opts.registry_log.clone(), clock.clone());
            let me = me.clone();
            let complain = Arc::new(log::Throttled::new(
                opts.registry_log.clone(),
                clock.clone(),
            ));
            // Only an engine this registry lists may hold a tunnel: its token, and its secret once bound.
            let verify: tunnel::hub::Verify = Arc::new(move |id, token, secret| {
                let ok = me
                    .upgrade()
                    .is_some_and(|c| c.verify_tunnel(id, token, secret));
                if !ok {
                    complain.say(
                        &format!("{id}|tunnel"),
                        &format!("refused a tunnel for engine {id}: not listed, or the wrong token or secret"),
                    );
                }
                ok
            });
            let hub = Hub::new(
                tunnel::hub::HubOptions {
                    verify,
                    render_key: opts.render_key,
                    ping_ms: opts.ping_ms,
                    uploads: opts.uploads,
                    min_protocol: opts.min_protocol,
                    log: opts.tunnel_log,
                },
                clock.clone(),
            );
            Core {
                registry,
                hub,
                tickets: Tickets::default(),
                clock,
                #[cfg(feature = "test-hooks")]
                verify_stub: Default::default(),
            }
        })
    }

    fn verify_tunnel(&self, id: &str, token: &str, secret: Option<&str>) -> bool {
        #[cfg(feature = "test-hooks")]
        if let Some(stub) = self
            .verify_stub
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            return stub(id, token, secret);
        }
        self.registry.verify_tunnel(id, token, secret)
    }
}
