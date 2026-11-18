//! account.rs — the driver of the members that belong to the account, not to its jobs: the engine's, onnx (3f, M9).
//! What a reader gets wrong: it wraps the jobs driver and hands every other member to it, so a front with no engine link still serves those.

use std::sync::Arc;
use std::time::{Duration, Instant};

use k7_file_service::verb_host::Error;

use super::{Answer, Checked, Driver, Member, devices, engine_ops, failure, onnx, plugins};
use crate::routes::files::service::native::engine::{EngineLink, Session};

/// What a member of the account needs: the engine link (none where that piece is off).
pub struct Account {
    base: Option<Arc<dyn Driver>>,
    link: Option<EngineLink>,
}

/// The driver of one request: the jobs driver's members stay its own.
pub fn with(base: Option<Arc<dyn Driver>>, link: Option<EngineLink>) -> Option<Arc<dyn Driver>> {
    Some(Arc::new(Account { base, link }))
}

fn off() -> axum::response::Response {
    failure(&Error::coded(
        503,
        "this host member is not served here",
        "off",
    ))
}

impl Driver for Account {
    fn call<'a>(&'a self, req: &'a Checked) -> Answer<'a> {
        Box::pin(async move {
            match req.member {
                Member::Onnx => match &self.link {
                    Some(l) => onnx::ask(req, l).await,
                    None => off(),
                },
                Member::Plugins
                | Member::PluginsRescan
                | Member::Devices
                | Member::DevicesStereo
                | Member::Engine => match &self.link {
                    Some(l) => match req.member {
                        Member::Plugins => plugins::list(req, l).await,
                        Member::PluginsRescan => plugins::rescan(req, l).await,
                        Member::Devices => devices::list(req, l).await,
                        Member::DevicesStereo => devices::stereo(req, l).await,
                        _ => engine_ops::run(req, l).await,
                    },
                    None => off(),
                },
                _ => match &self.base {
                    Some(b) => b.call(req).await,
                    None => off(),
                },
            }
        })
    }
}

/// How long a query waits for a link still being dialled.
pub const QUERY_WAIT: Duration = Duration::from_secs(2);

/// The workspace's session once its socket is open, waiting up to `wait` for one being dialled; none with no engine bound.
pub async fn ready(link: &EngineLink, root: &str, wait: Duration) -> Option<Arc<Session>> {
    link.binding().url.as_ref()?;
    let session = link.session(Some(root));
    let end = Instant::now() + wait;
    loop {
        if session.dial_ready() {
            return Some(session);
        }
        if Instant::now() >= end {
            return None;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}
