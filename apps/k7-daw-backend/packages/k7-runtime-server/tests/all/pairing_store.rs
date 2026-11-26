//! pairing_store.rs — the store's behaviours, and the file against the format every installed engine wrote.

use k7_runtime_server::pairing::net::{is_loopback, is_private, origin_allowed, parse_origins};
use k7_runtime_server::pairing::store::Pairing;
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

const PHONE: u64 = 0x10;
const LAPTOP: u64 = 0x20;

/// A store path of its own in the temp folder, nothing on it yet.
pub fn temp_store(tag: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "k7-pairing-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{tag}.json"))
}

fn fixture(name: &str) -> String {
    super::repo_file(&format!(
        "apps/k7-daw-backend/packages/k7-runtime-server/fixtures/{name}"
    ))
}

#[test]
fn loopback_is_the_machine_and_nothing_else() {
    for ip in ["127.0.0.1", "127.4.5.6", "::1", "::ffff:127.0.0.1"] {
        assert!(is_loopback(ip), "{ip}");
    }
    for ip in [
        "192.168.1.20",
        "10.0.0.1",
        "::ffff:192.168.1.20",
        "fe80::1",
        "",
    ] {
        assert!(!is_loopback(ip), "{ip}");
    }
}

#[test]
fn private_is_the_lan_and_nothing_else() {
    for ip in [
        "192.168.1.20",
        "10.0.0.1",
        "172.16.0.5",
        "172.31.255.1",
        "169.254.3.4",
        "::ffff:192.168.1.20",
        "fe80::1",
        "FE80::abcd",
        "fd12:3456::1",
        "fc00::2",
    ] {
        assert!(is_private(ip), "{ip}");
    }
    for ip in [
        "8.8.8.8",
        "172.32.0.1",
        "172.15.0.1",
        "192.169.0.1",
        "100.64.0.1",
        "2001:db8::1",
        "::ffff:8.8.8.8",
        "127.0.0.1",
        "::1",
        "",
        "256.1.1.1",
        "10.0.0.1x",
    ] {
        assert!(!is_private(ip), "{ip}");
    }
}

#[test]
fn only_the_studio_origins_open_a_socket() {
    let none: Vec<String> = Vec::new();
    for ok in [
        "http://127.0.0.1:18789",
        "http://localhost:5173",
        "http://[::1]:8080",
        "https://localhost",
        "https://app.k7.rocks",
        "https://app.priv.k7.rocks",
        "https://docs.k7.rocks",
        "HTTPS://APP.K7.ROCKS/",
        "ws://127.0.0.1:8765",
        "wss://192.168.1.5:8765",
    ] {
        assert!(origin_allowed(ok, &none), "{ok}");
    }
    for bad in [
        "https://evil.example",
        "https://k7.rocks.evil.example",
        "https://evilk7.rocks",
        "http://app.k7.rocks",
        "https://app.k7.rocks:8443",
        "https://user@k7.rocks",
        "https://st0.app",
        "null",
        "",
        "file://",
        "http://127.0.0.1.evil.example",
        "http://localhost:abc",
        "http://[::1]x",
    ] {
        assert!(!origin_allowed(bad, &none), "{bad}");
    }
    let extra = parse_origins(" https://studio.example.org/ ,http://studio.lan:8080,, ");
    assert_eq!(extra.len(), 2);
    assert!(origin_allowed("https://studio.example.org", &extra));
    assert!(origin_allowed("http://studio.lan:8080", &extra));
    assert!(!origin_allowed("http://studio.lan:8081", &extra));
}

#[test]
fn an_approved_request_yields_a_key_that_verifies() {
    let p = Pairing::open(temp_store("a"));
    let r = p
        .request("iPhone · Safari", "192.168.1.20", Some(PHONE))
        .unwrap();
    assert_eq!(r.code.len(), 6);
    assert_eq!(p.pending().len(), 1);
    let a = p.approve(&r.id).unwrap();
    assert_eq!(a.from, Some(PHONE));
    assert_eq!(a.key.len(), 64);
    assert!(p.pending().is_empty());
    assert!(p.verify(&a.key, Some(LAPTOP)).is_some());
    assert!(p.verify(&"0".repeat(64), Some(LAPTOP)).is_none());
    assert!(p.verify("", Some(LAPTOP)).is_none());
    assert!(p.approve(&r.id).is_none(), "answered once");
}

#[test]
fn a_denied_request_is_gone() {
    let p = Pairing::open(temp_store("b"));
    let r = p.request("tablet", "192.168.1.21", Some(PHONE)).unwrap();
    assert_eq!(p.deny(&r.id), Some(PHONE));
    assert!(p.pending().is_empty());
    assert!(p.approve(&r.id).is_none());
}

#[test]
fn asking_again_replaces_the_request() {
    let p = Pairing::open(temp_store("c"));
    let first = p.request("phone", "192.168.1.20", Some(PHONE)).unwrap();
    let second = p.request("phone", "192.168.1.20", Some(PHONE)).unwrap();
    assert_eq!(p.pending().len(), 1);
    assert!(p.approve(&first.id).is_none());
    assert!(p.approve(&second.id).is_some());
}

#[test]
fn the_room_cannot_queue_without_bound() {
    let p = Pairing::open(temp_store("d"));
    for i in 0..8 {
        assert!(p.request("d", "192.168.1.30", Some(0x100 + i)).is_some());
    }
    assert!(
        p.request("one too many", "192.168.1.31", Some(LAPTOP))
            .is_none()
    );
}

#[test]
fn a_closed_socket_takes_its_request_with_it() {
    let p = Pairing::open(temp_store("e"));
    p.request("phone", "192.168.1.20", Some(PHONE));
    assert!(p.forget(PHONE));
    assert!(p.pending().is_empty());
    assert!(!p.forget(PHONE));
}

#[test]
fn a_pairing_survives_a_restart_and_a_revoke_ends_it() {
    let file = temp_store("f");
    let (key, id) = {
        let p = Pairing::open(&file);
        let a = p
            .approve(&p.request("phone", "192.168.1.20", Some(PHONE)).unwrap().id)
            .unwrap();
        (a.key, a.device.id)
    };
    let again = Pairing::open(&file);
    assert_eq!(again.devices().len(), 1);
    assert!(again.verify(&key, Some(LAPTOP)).is_some());
    assert_eq!(
        again.revoke(&id),
        vec![LAPTOP],
        "its open connection, to be closed"
    );
    assert!(again.verify(&key, Some(PHONE)).is_none());
    assert!(Pairing::open(&file).devices().is_empty());
}

#[test]
fn an_unreadable_store_pairs_nobody() {
    let file = temp_store("g");
    std::fs::write(&file, "{ not json").unwrap();
    assert!(Pairing::open(&file).devices().is_empty());
    // Anything that is not the store's own shape pairs nobody either.
    std::fs::write(
        &file,
        r#"{"devices":[{"id":"a1","name":"x","key":"k","created":"no"}]}"#,
    )
    .unwrap();
    assert!(Pairing::open(&file).devices().is_empty());
}

#[test]
fn a_key_too_short_or_a_device_without_an_id_is_not_paired() {
    let file = temp_store("h");
    let good = "a".repeat(64);
    let text = format!(
        r#"{{"devices":[{{"id":"","key":"{good}"}},{{"id":"s","key":"short"}},{{"id":"ok","key":"{good}","name":"n","created":5,"lastSeen":6}}]}}"#
    );
    std::fs::write(&file, text).unwrap();
    let d = Pairing::open(&file).devices();
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].id.as_str(), d[0].created_ms, d[0].last_seen_ms),
        ("ok", 5, 6)
    );
}

#[test]
fn a_name_is_clipped_to_64_bytes_and_a_cut_character_is_replaced() {
    let p = Pairing::open(temp_store("i"));
    let long = format!("{}é", "a".repeat(63));
    assert_eq!(
        p.request(&long, "10.0.0.1", Some(1)).unwrap().name,
        format!("{}\u{fffd}", "a".repeat(63))
    );
    assert_eq!(p.request("", "10.0.0.1", Some(2)).unwrap().name, "a device");
}

#[test]
fn the_file_is_the_c_plus_plus_stores_byte_for_byte() {
    for name in ["paired-devices.two.json", "paired-devices.empty.json"] {
        let file = temp_store("fixture");
        std::fs::write(&file, fixture(name)).unwrap();
        let p = Pairing::open(&file);
        // A change rewrites the whole file: approving and revoking one more leaves the fixture's bytes.
        let before = p.devices();
        let a = p
            .approve(&p.request("tmp", "10.0.0.2", None).unwrap().id)
            .unwrap();
        p.revoke(&a.device.id);
        assert_eq!(p.devices(), before);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            fixture(name),
            "{name}"
        );
    }
}

#[test]
fn the_two_devices_the_c_plus_plus_store_wrote_are_read_and_their_keys_verify() {
    let file = temp_store("fixture2");
    std::fs::write(&file, fixture("paired-devices.two.json")).unwrap();
    let p = Pairing::open(&file);
    let d = p.devices();
    assert_eq!(d.len(), 2);
    assert_eq!(d[0].name, "Pixel · é \"Chrome\"\n\\");
    let key = "a161d00e5c2473644ac31c2a074848e4be7ffb2a6ac6a541bbfbb94de186e243";
    assert_eq!(p.verify(key, None).unwrap().id, "d74ea8f4");
}

#[cfg(unix)]
#[test]
fn the_file_is_readable_by_its_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let file = temp_store("perm");
    let p = Pairing::open(&file);
    p.approve(&p.request("x", "10.0.0.2", None).unwrap().id)
        .unwrap();
    assert_eq!(
        std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(
        !file.with_extension("json.tmp").exists(),
        "written aside and renamed"
    );
}

static NOW: AtomicI64 = AtomicI64::new(1_000_000);
fn clock() -> i64 {
    NOW.load(Ordering::SeqCst)
}

#[test]
fn a_request_expires_after_two_minutes_and_last_seen_is_saved_hourly() {
    let file = temp_store("clock");
    let p = Pairing::with_clock(&file, clock);
    let r = p.request("phone", "10.0.0.3", Some(PHONE)).unwrap();
    NOW.fetch_add(2 * 60 * 1000, Ordering::SeqCst);
    assert_eq!(p.pending().len(), 1, "exactly two minutes still waits");
    NOW.fetch_add(1, Ordering::SeqCst);
    assert!(p.pending().is_empty());
    assert!(p.approve(&r.id).is_none());

    let a = p
        .approve(&p.request("phone", "10.0.0.3", None).unwrap().id)
        .unwrap();
    let saved = std::fs::read_to_string(&file).unwrap();
    NOW.fetch_add(60 * 1000, Ordering::SeqCst);
    p.verify(&a.key, None).unwrap();
    assert_eq!(
        std::fs::read_to_string(&file).unwrap(),
        saved,
        "a minute later is not saved"
    );
    NOW.fetch_add(61 * 60 * 1000, Ordering::SeqCst);
    p.verify(&a.key, None).unwrap();
    assert_ne!(std::fs::read_to_string(&file).unwrap(), saved);
    assert_eq!(Pairing::open(&file).devices()[0].last_seen_ms, clock());
}
