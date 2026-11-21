//! k7-lan — how k7 devices on one network find each other: one `Discovery` trait, three ways to fill it.
//! `MdnsSd` advertises and browses `_k7._tcp` (desktop, Android), `HostBridge` is what iOS' own browser feeds,
//! `Static` is a fixed list for beds and tests. Choices: `apps/k7-daw-backend/docs/lan.md`.
//! What a reader gets wrong: a peer here is only an address and a claim; the front's `hello` is what says who it is.

mod bridge;
mod mdns;
mod statik;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use bridge::{AdvertiseFn, HostBridge, Pushed, host_open, set_host_open};
pub use mdns::MdnsSd;
pub use statik::Static;

/// The service type every k7 device advertises while it is on the network.
pub const SERVICE: &str = "_k7._tcp.local.";

/// A device found on the network: where its door and its engine listen, and what its record claimed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    /// The engine id the record carried; empty when the source does not know it (a fixed list).
    pub engine_id: String,
    /// The device name the record carried; empty when unknown.
    pub name: String,
    /// An address the peer answers on.
    pub host: String,
    /// The door's port.
    pub door: u16,
    /// The engine's port.
    pub engine: u16,
    /// The door's version, 0 when unknown.
    pub api: u32,
    /// The engine protocol, 0 when unknown.
    pub proto: u32,
}

/// What this device says about itself while it is on the network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Advert {
    pub engine_id: String,
    pub name: String,
    pub door: u16,
    pub engine: u16,
    pub api: u32,
    pub proto: u32,
}

impl Advert {
    /// The TXT record: `id`, `name`, `door`, `engine`, `api`, `proto`.
    pub fn txt(&self) -> BTreeMap<String, String> {
        BTreeMap::from([
            ("id".into(), self.engine_id.clone()),
            ("name".into(), self.name.clone()),
            ("door".into(), self.door.to_string()),
            ("engine".into(), self.engine.to_string()),
            ("api".into(), self.api.to_string()),
            ("proto".into(), self.proto.to_string()),
        ])
    }
}

/// Reads a peer out of a TXT record and the address it came from; None when the record is not a k7 door's.
pub fn peer_of(host: &str, txt: &dyn Fn(&str) -> Option<String>) -> Option<Peer> {
    let num = |k: &str| txt(k).and_then(|v| v.parse::<u32>().ok());
    let door = u16::try_from(num("door")?).ok()?;
    let engine = u16::try_from(num("engine")?).ok()?;
    Some(Peer {
        engine_id: txt("id").unwrap_or_default(),
        name: txt("name").unwrap_or_default(),
        host: host.to_string(),
        door,
        engine,
        api: num("api").unwrap_or(0),
        proto: num("proto").unwrap_or(0),
    })
}

/// A source of peers and the one place this device says it is here.
pub trait Discovery: Send + Sync {
    /// Starts saying this device is here, or stops when `None`; idempotent.
    fn advertise(&self, advert: Option<&Advert>);
    /// The peers heard now, this device's own record excluded by the caller.
    fn peers(&self) -> Vec<Peer>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_txt_record_reads_back_as_the_peer_it_was_written_from() {
        let advert = Advert {
            engine_id: "e1".into(),
            name: "Studio".into(),
            door: 18790,
            engine: 8765,
            api: 1,
            proto: 7,
        };
        let txt = advert.txt();
        let peer = peer_of("192.168.1.9", &|k| txt.get(k).cloned()).expect("a peer");
        assert_eq!(peer.engine_id, "e1");
        assert_eq!(
            (peer.door, peer.engine, peer.api, peer.proto),
            (18790, 8765, 1, 7)
        );
        assert_eq!(peer.host, "192.168.1.9");
    }

    #[test]
    fn a_record_with_no_door_is_not_a_k7_peer() {
        assert!(peer_of("10.0.0.2", &|k| (k == "id").then(|| "x".into())).is_none());
        assert!(
            peer_of("10.0.0.2", &|k| match k {
                "door" => Some("70000".into()),
                "engine" => Some("1".into()),
                _ => None,
            })
            .is_none()
        );
    }
}
