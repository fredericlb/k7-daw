//! files.rs — the state folder's files: the identity is kept and its secret gained, the merged log is set aside
//! when large, the pidfile reads both its formats, and the host lock is refused while refreshed, taken when stale.

use std::time::{Duration, SystemTime};

use k7_supervisor::host_lock::{Holder, HostLock};
use k7_supervisor::identity;
use k7_supervisor::log::{Log, rotate};
use k7_supervisor::pidfile::{self, Record};

use crate::scratch::Scratch;

#[test]
fn an_identity_is_minted_once_and_an_old_one_gains_a_secret() {
    let dir = Scratch::new("id");
    let path = dir.path().join("state/engine-id.json");
    let first = identity::ensure(&path);
    assert_eq!(first.id.len(), 36);
    assert_eq!(&first.id[14..15], "4");
    assert_eq!((first.token.len(), first.tunnel_secret.len()), (48, 64));
    assert_eq!(identity::ensure(&path), first);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
    std::fs::write(&path, r#"{"id":"old","token":"tok","createdAt":"2025"}"#).unwrap();
    let kept = identity::ensure(&path);
    assert_eq!(
        (
            kept.id.as_str(),
            kept.token.as_str(),
            kept.created_at.as_str()
        ),
        ("old", "tok", "2025")
    );
    assert_eq!(kept.tunnel_secret.len(), 64);
    assert_eq!(identity::ensure(&path), kept);
}

#[test]
fn a_large_log_is_set_aside_and_a_line_has_its_format() {
    let dir = Scratch::new("log");
    let path = dir.path().join(".studio.log");
    std::fs::write(&path, vec![b'x'; 100]).unwrap();
    assert!(!rotate(&path, 1000));
    assert!(rotate(&path, 10));
    assert!(dir.path().join(".studio.log.1").exists() && !path.exists());
    let mut log = Log::new(path.clone(), false, 60);
    log.line("host", "one");
    log.line("engine", "two");
    let text = std::fs::read_to_string(&path).unwrap();
    let first = text.lines().next().unwrap();
    assert_eq!(&first[8..], " [host] one");
    assert!(first[..8].chars().all(|c| c.is_ascii_digit() || c == ':'));
    // A line past the limit moves the file aside and starts a fresh one.
    log.line("host", "a line long enough to pass sixty bytes");
    assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
}

#[test]
fn a_pidfile_reads_both_formats() {
    let bare = pidfile::parse("4242\n");
    assert_eq!((bare.pid, bare.verified()), (4242, false));
    let r = Record {
        pid: 7,
        start: Some("1700000000".into()),
        exe: Some("/x/k7-runtime".into()),
        port: 8766,
    };
    assert_eq!(pidfile::parse(&pidfile::format(&r)), r);
    for junk in ["", "abc", "-3", "1234567890"] {
        assert_eq!(pidfile::parse(junk).pid, 0, "{junk}");
    }
    assert_eq!(pidfile::parse("9\nport=99999\n").port, 0);
    assert!(
        pidfile::is_engine_exe("C:\\k7\\K7-Runtime.exe")
            && !pidfile::is_engine_exe("/bin/k7-runtimes")
    );
}

/// A live pid that is not ours: the init process on every Unix.
fn foreign_live_pid() -> u32 {
    1
}

fn age(path: &std::path::Path, secs: u64) {
    let f = std::fs::File::options().write(true).open(path).unwrap();
    f.set_modified(SystemTime::now() - Duration::from_secs(secs))
        .unwrap();
}

#[cfg(unix)]
#[test]
fn the_host_lock_is_refused_while_refreshed_and_taken_when_stale() {
    let dir = Scratch::new("lock");
    let lock = HostLock::new(dir.path().join("engine-host.pid"));
    std::fs::write(&lock.path, format!("{}\n", foreign_live_pid())).unwrap();
    assert_eq!(lock.acquire(), Holder::Pid(foreign_live_pid()));
    age(&lock.path, 20);
    assert_eq!(lock.acquire(), Holder::Nobody);
    assert_eq!(
        std::fs::read_to_string(&lock.path).unwrap().trim(),
        lock.pid.to_string()
    );
    // A lock that came to name somebody else is neither overwritten nor released.
    std::fs::write(&lock.path, format!("{}\n", foreign_live_pid())).unwrap();
    lock.refresh();
    lock.release();
    assert_eq!(std::fs::read_to_string(&lock.path).unwrap().trim(), "1");
    // A fresh lock naming nobody is a supervisor between its create and its write.
    std::fs::write(&lock.path, "").unwrap();
    assert_eq!(lock.acquire(), Holder::Unknown);
    age(&lock.path, 20);
    assert_eq!(lock.acquire(), Holder::Nobody);
    lock.release();
    assert!(!lock.path.exists());
}
