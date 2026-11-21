//! mdns.rs — `_k7._tcp` over `mdns-sd`: one daemon that advertises this device and browses the others.
//! What a reader gets wrong: a peer is dropped when its record goes, not when it stops answering; the front's hello says that.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};

use crate::{Advert, Discovery, Peer, SERVICE, peer_of};

type Heard = Arc<Mutex<HashMap<String, Peer>>>;

pub struct MdnsSd {
    daemon: ServiceDaemon,
    heard: Heard,
    /// The fullname this device registered, to unregister it.
    mine: Mutex<Option<(Advert, String)>>,
}

impl MdnsSd {
    /// Starts the daemon and the browse; an error is the socket the daemon could not bind (no multicast).
    pub fn start() -> Result<MdnsSd, String> {
        let daemon = ServiceDaemon::new().map_err(|e| format!("mdns: {e}"))?;
        let events = daemon
            .browse(SERVICE)
            .map_err(|e| format!("mdns browse: {e}"))?;
        let heard: Heard = Arc::default();
        let sink = heard.clone();
        // A plain thread: the receiver is flume's blocking one, and the daemon owns its own threads too.
        std::thread::Builder::new()
            .name("k7-lan-browse".into())
            .spawn(move || {
                while let Ok(event) = events.recv() {
                    let mut heard = sink.lock().unwrap_or_else(|p| p.into_inner());
                    match event {
                        ServiceEvent::ServiceResolved(info) => {
                            let Some(host) = info.get_addresses_v4().into_iter().next() else {
                                continue;
                            };
                            let txt = |k: &str| info.get_property_val_str(k).map(str::to_string);
                            if let Some(peer) = peer_of(&host.to_string(), &txt) {
                                heard.insert(info.get_fullname().to_string(), peer);
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            heard.remove(&fullname);
                        }
                        _ => {}
                    }
                }
            })
            .map_err(|e| format!("mdns browse thread: {e}"))?;
        Ok(MdnsSd {
            daemon,
            heard,
            mine: Mutex::new(None),
        })
    }

    fn record(advert: &Advert) -> Result<ServiceInfo, String> {
        let host = format!("k7-{}.local.", advert.engine_id);
        let instance = format!("k7-{}", advert.engine_id);
        Ok(ServiceInfo::new(
            SERVICE,
            &instance,
            &host,
            "",
            advert.door,
            advert.txt().into_iter().collect::<HashMap<_, _>>(),
        )
        .map_err(|e| format!("mdns record: {e}"))?
        .enable_addr_auto())
    }
}

impl Discovery for MdnsSd {
    fn advertise(&self, advert: Option<&Advert>) {
        let mut mine = self.mine.lock().unwrap_or_else(|p| p.into_inner());
        if mine.as_ref().map(|(a, _)| a) == advert {
            return;
        }
        if let Some((_, fullname)) = mine.take() {
            let _ = self.daemon.unregister(&fullname);
        }
        let Some(advert) = advert else { return };
        match Self::record(advert).and_then(|info| {
            let fullname = info.get_fullname().to_string();
            self.daemon
                .register(info)
                .map(|()| fullname)
                .map_err(|e| format!("mdns register: {e}"))
        }) {
            Ok(fullname) => *mine = Some((advert.clone(), fullname)),
            Err(e) => eprintln!("k7-lan: not advertising: {e}"),
        }
    }

    fn peers(&self) -> Vec<Peer> {
        let heard = self.heard.lock().unwrap_or_else(|p| p.into_inner());
        let mut peers: Vec<Peer> = heard.values().cloned().collect();
        peers.sort_by(|a, b| (&a.name, &a.engine_id).cmp(&(&b.name, &b.engine_id)));
        peers
    }
}

impl Drop for MdnsSd {
    fn drop(&mut self) {
        let _ = self.daemon.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs multicast on the machine, which a sandbox or a CI container may not have: run by hand.
    #[test]
    #[ignore = "needs multicast"]
    fn an_advert_is_browsed_back_on_the_same_machine() {
        let a = MdnsSd::start().expect("daemon");
        a.advertise(Some(&Advert {
            engine_id: "lan-test".into(),
            name: "Test".into(),
            door: 18790,
            engine: 8765,
            api: 1,
            proto: 1,
        }));
        let b = MdnsSd::start().expect("daemon");
        let found = (0..50).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(200));
            b.peers().iter().any(|p| p.engine_id == "lan-test")
        });
        assert!(found, "the advert was not browsed within 10 s");
    }
}
