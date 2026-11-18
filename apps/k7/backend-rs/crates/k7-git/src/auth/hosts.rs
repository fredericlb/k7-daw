//! hosts.rs — the SSH servers this k7 account trusts: host and port to the key's fingerprint, in one file beside the
//! account's settings. What a reader gets wrong: a fingerprint reaches the file only when the person trusted it, and a
//! key that differs from the listed one is refused, never replaced; forgetting is the person's act.

use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

pub const HOSTS_FILE: &str = ".k7-known-hosts.json";

/// The trusted servers of one account; a file where the account keeps its settings, or memory for the tests.
#[derive(Default)]
pub struct KnownHosts {
    path: Option<PathBuf>,
    memory: Mutex<BTreeMap<String, String>>,
    /// Held across a read-modify-write so two shares of one server cannot lose each other's line.
    write: Mutex<()>,
}

fn name(host: &str, port: u16) -> String {
    format!("{}:{port}", host.trim().to_lowercase())
}

impl KnownHosts {
    /// The list in `<folder>/.k7-known-hosts.json`.
    pub fn in_folder(folder: &std::path::Path) -> KnownHosts {
        KnownHosts {
            path: Some(folder.join(HOSTS_FILE)),
            ..KnownHosts::default()
        }
    }

    /// A list that lives and dies with the process.
    pub fn memory() -> KnownHosts {
        KnownHosts::default()
    }

    /// A missing file is an empty list; an unreadable one is an error and never "trusts nothing".
    fn load(&self) -> io::Result<BTreeMap<String, String>> {
        let Some(path) = &self.path else {
            return Ok(self
                .memory
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone());
        };
        match std::fs::read(path) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| {
                io::Error::new(io::ErrorKind::InvalidData, format!("{HOSTS_FILE}: {e}"))
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(BTreeMap::new()),
            Err(e) => Err(e),
        }
    }

    fn store(&self, list: BTreeMap<String, String>) -> io::Result<()> {
        let Some(path) = &self.path else {
            *self.memory.lock().unwrap_or_else(|e| e.into_inner()) = list;
            return Ok(());
        };
        let text = serde_json::to_string_pretty(&list).map_err(io::Error::other)?;
        k7_sync::fsx::write_atomic(path, &text)
    }

    /// The fingerprint trusted for `host:port`, if any.
    pub fn get(&self, host: &str, port: u16) -> io::Result<Option<String>> {
        Ok(self.load()?.get(&name(host, port)).cloned())
    }

    /// Trusts `fingerprint` for `host:port`; a different one already listed stays and is the caller's refusal.
    pub fn remember(&self, host: &str, port: u16, fingerprint: &str) -> io::Result<()> {
        let _one = self.write.lock().unwrap_or_else(|e| e.into_inner());
        let mut list = self.load()?;
        if list.contains_key(&name(host, port)) {
            return Ok(());
        }
        list.insert(name(host, port), fingerprint.to_string());
        self.store(list)
    }

    /// Takes `host:port` off the list; whether it was there.
    pub fn forget(&self, host: &str, port: u16) -> io::Result<bool> {
        let _one = self.write.lock().unwrap_or_else(|e| e.into_inner());
        let mut list = self.load()?;
        let was = list.remove(&name(host, port)).is_some();
        if was {
            self.store(list)?;
        }
        Ok(was)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("k7-hosts-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_trusted_key_is_found_by_the_next_run_and_the_host_is_read_without_case() {
        let d = dir("persist");
        KnownHosts::in_folder(&d)
            .remember("NAS.local", 22, "SHA256:a")
            .unwrap();
        let back = KnownHosts::in_folder(&d);
        assert_eq!(
            back.get("nas.LOCAL", 22).unwrap().as_deref(),
            Some("SHA256:a")
        );
        assert_eq!(back.get("nas.local", 2222).unwrap(), None);
    }

    #[test]
    fn remembering_never_replaces_a_listed_key_and_forgetting_is_the_only_way_out() {
        let hosts = KnownHosts::memory();
        hosts.remember("h", 22, "SHA256:old").unwrap();
        hosts.remember("h", 22, "SHA256:new").unwrap();
        assert_eq!(hosts.get("h", 22).unwrap().as_deref(), Some("SHA256:old"));
        assert!(hosts.forget("h", 22).unwrap());
        assert!(!hosts.forget("h", 22).unwrap());
        assert_eq!(hosts.get("h", 22).unwrap(), None);
    }

    #[test]
    fn a_list_that_cannot_be_read_is_an_error_and_never_an_empty_list() {
        let d = dir("corrupt");
        std::fs::write(d.join(HOSTS_FILE), "{ not json").unwrap();
        let hosts = KnownHosts::in_folder(&d);
        assert!(hosts.get("h", 22).is_err());
        assert!(hosts.remember("h", 22, "SHA256:a").is_err());
        assert_eq!(
            std::fs::read_to_string(d.join(HOSTS_FILE)).unwrap(),
            "{ not json"
        );
    }
}
