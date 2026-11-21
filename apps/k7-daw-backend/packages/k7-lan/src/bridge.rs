//! bridge.rs — discovery done by the host: iOS browses and advertises through Network.framework, because raw
//! multicast there needs an entitlement Apple grants apart. The host calls `Pushed::set_peers` with what it
//! heard and is called back through `advertise` with what to say; neither callback may call back into the front.

use std::ffi::{CString, c_char};
use std::sync::{Arc, Mutex, OnceLock};

use crate::{Advert, Discovery, Peer};

/// Tells the host to advertise this JSON record (`{"id","name","door","engine","api","proto"}`), or to stop on null.
pub type AdvertiseFn = extern "C" fn(record: *const c_char);

/// The peers the host pushed, shared with whoever the front built for them.
#[derive(Clone, Default)]
pub struct Pushed(Arc<Mutex<Vec<Peer>>>);

impl Pushed {
    /// Replaces the list with the host's JSON array of peers; a malformed list leaves the last one.
    pub fn set_peers(&self, json: &str) -> bool {
        match serde_json::from_str::<Vec<Peer>>(json) {
            Ok(peers) => {
                *self.0.lock().unwrap_or_else(|p| p.into_inner()) = peers;
                true
            }
            Err(_) => false,
        }
    }
}

/// Whether a host that runs the supervisor itself (a phone) says the LAN is open, which the front cannot read there.
static HOST_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The host's word on the LAN switch.
pub fn set_host_open(open: bool) {
    HOST_OPEN.store(open, std::sync::atomic::Ordering::SeqCst);
}

/// What the host last said; false until it says.
pub fn host_open() -> bool {
    HOST_OPEN.load(std::sync::atomic::Ordering::SeqCst)
}

/// What the host installed before the front started: the one bridge a process has.
static INSTALLED: OnceLock<(AdvertiseFn, Pushed)> = OnceLock::new();

pub struct HostBridge {
    advertise: AdvertiseFn,
    pushed: Pushed,
}

impl HostBridge {
    /// Installs the host's advertise callback, once; the returned handle takes its peers.
    pub fn install(advertise: AdvertiseFn) -> Pushed {
        let (_, pushed) = INSTALLED.get_or_init(|| (advertise, Pushed::default()));
        pushed.clone()
    }

    /// The bridge the host installed, if one did.
    pub fn installed() -> Option<HostBridge> {
        INSTALLED.get().map(|(advertise, pushed)| HostBridge {
            advertise: *advertise,
            pushed: pushed.clone(),
        })
    }
}

impl HostBridge {
    /// Takes the host's browse result, as `Pushed::set_peers` does.
    pub fn push(&self, json: &str) -> bool {
        self.pushed.set_peers(json)
    }
}

impl Discovery for HostBridge {
    fn advertise(&self, advert: Option<&Advert>) {
        match advert {
            None => (self.advertise)(std::ptr::null()),
            Some(a) => {
                let record = serde_json::json!({
                    "id": a.engine_id, "name": a.name, "door": a.door,
                    "engine": a.engine, "api": a.api, "proto": a.proto,
                })
                .to_string();
                if let Ok(c) = CString::new(record) {
                    (self.advertise)(c.as_ptr());
                }
            }
        }
    }

    fn peers(&self) -> Vec<Peer> {
        self.pushed
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushed_peers_replace_the_list_and_a_bad_push_keeps_it() {
        let pushed = Pushed::default();
        assert!(pushed.set_peers(
            r#"[{"engineId":"a","name":"Mac","host":"10.0.0.2","door":18790,"engine":8765,"api":1,"proto":1}]"#
        ));
        assert!(!pushed.set_peers("not json"));
        extern "C" fn nothing(_: *const c_char) {}
        let bridge = HostBridge {
            advertise: nothing,
            pushed,
        };
        assert_eq!(bridge.peers().len(), 1);
    }
}
