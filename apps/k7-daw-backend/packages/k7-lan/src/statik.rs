//! statik.rs — a fixed peer list, read from `K7_LAN_PEERS=host:door:engine,…`, for beds and tests where no multicast runs.
//! Advertising does nothing: the list is all there is.

use crate::{Advert, Discovery, Peer};

#[derive(Debug)]
pub struct Static(Vec<Peer>);

impl Static {
    /// `host:door:engine` entries, comma separated; an entry that does not parse is an error, not a silent gap.
    pub fn parse(list: &str) -> Result<Static, String> {
        let mut peers = Vec::new();
        for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
            let parts: Vec<&str> = entry.rsplitn(3, ':').collect();
            let [engine, door, host] = parts.as_slice() else {
                return Err(format!(
                    "K7_LAN_PEERS entry {entry:?}: want host:door:engine"
                ));
            };
            let port = |p: &str| {
                p.parse::<u16>()
                    .map_err(|e| format!("K7_LAN_PEERS entry {entry:?}: {e}"))
            };
            if host.is_empty() {
                return Err(format!("K7_LAN_PEERS entry {entry:?}: no host"));
            }
            peers.push(Peer {
                engine_id: String::new(),
                name: String::new(),
                host: (*host).to_string(),
                door: port(door)?,
                engine: port(engine)?,
                api: 0,
                proto: 0,
            });
        }
        Ok(Static(peers))
    }
}

impl Discovery for Static {
    fn advertise(&self, _advert: Option<&Advert>) {}

    fn peers(&self) -> Vec<Peer> {
        self.0.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_parses_into_peers() {
        let s = Static::parse("127.0.0.1:28791:8765, 10.0.0.4:18790:9000").unwrap();
        let peers = s.peers();
        assert_eq!(peers.len(), 2);
        assert_eq!(
            (peers[0].host.as_str(), peers[0].door, peers[0].engine),
            ("127.0.0.1", 28791, 8765)
        );
        assert_eq!(peers[1].host, "10.0.0.4");
    }

    #[test]
    fn a_bad_entry_is_refused_by_name() {
        assert!(Static::parse("host:1").unwrap_err().contains("host:1"));
        assert!(Static::parse("h:x:2").is_err());
        assert!(Static::parse(":1:2").is_err());
        assert!(Static::parse("").unwrap().peers().is_empty());
    }
}
