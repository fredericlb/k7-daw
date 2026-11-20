//! A sidecar is written whole and read for what it is (2414, 2415): never half a file at the final name, a link replaced
//! and not followed, and an unreadable sidecar told apart from a stale one.

use std::path::{Path, PathBuf};

use k7_mediatek::sidecar::{
    Envelope, cache_path, check_read, read_checked, serialise, write_atomic,
};

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("k7-sidecar-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn env(buckets: usize) -> Envelope {
    Envelope {
        fingerprint: "4-0123456789abcdef".into(),
        sample_rate: 8000.0,
        channels: 1.0,
        frames: 32000.0,
        seconds: 4.0,
        peak: vec![0.5; buckets],
        rms: vec![0.25; buckets],
        transients: vec![0.5],
        sensitivity: 2.0,
        bpm: 120.0,
        low: Some(vec![]),
        mid: Some(vec![]),
        high: Some(vec![]),
    }
}

fn source(d: &Path) -> PathBuf {
    let s = d.join("a.wav");
    std::fs::write(&s, b"RIFF").unwrap();
    s
}

/// A directory the owner cannot write to is writable anyway for root, which these tests then skip.
#[cfg(unix)]
fn locked(path: &Path, mode: u32) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    let probe = path.join(".probe");
    let blocked = std::fs::File::create(&probe).is_err();
    let _ = std::fs::remove_file(&probe);
    blocked
}

#[test]
fn a_sidecar_is_written_whole_and_no_temporary_is_left() {
    let d = dir("whole");
    let s = source(&d);
    write_atomic(&s, &env(8)).unwrap();
    assert_eq!(read_checked(&s).unwrap(), Some(env(8)));
    let mut names: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["a.wav", "a.wav.cache"]);
    assert_eq!(
        std::fs::read_to_string(cache_path(&s)).unwrap(),
        serialise(&env(8))
    );
}

#[test]
fn a_reader_never_sees_half_a_sidecar_at_the_final_name() {
    let d = dir("torn");
    let s = source(&d);
    write_atomic(&s, &env(4000)).unwrap();
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|t| {
        t.spawn(|| {
            for n in 0..150 {
                write_atomic(&s, &env(3000 + n)).unwrap();
            }
            done.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        let mut reads = 0;
        while !done.load(std::sync::atomic::Ordering::Relaxed) {
            assert!(
                read_checked(&s).unwrap().is_some(),
                "read {reads} met a torn sidecar"
            );
            reads += 1;
        }
    });
}

#[cfg(unix)]
#[test]
fn a_link_at_the_sidecars_name_is_replaced_not_written_through() {
    let d = dir("link");
    let s = source(&d);
    let outside = d.join("outside.txt");
    std::fs::write(&outside, "mine").unwrap();
    std::os::unix::fs::symlink(&outside, cache_path(&s)).unwrap();
    // A link is stale to a reader, so nothing outside is ever parsed as a sidecar.
    assert_eq!(check_read(&s).unwrap().env, None);
    write_atomic(&s, &env(2)).unwrap();
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "mine");
    assert!(
        !std::fs::symlink_metadata(cache_path(&s))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(read_checked(&s).unwrap(), Some(env(2)));
}

#[cfg(unix)]
#[test]
fn a_failed_write_leaves_no_temporary_beside_the_source() {
    let d = dir("failed");
    let s = source(&d);
    if locked(&d, 0o555) {
        assert!(write_atomic(&s, &env(2)).is_err());
        assert!(!cache_path(&s).exists());
        let n = std::fs::read_dir(&d).unwrap().count();
        assert_eq!(n, 1, "only the source is left");
    }
    locked(&d, 0o755);
    // The rename's own failure: a directory at the final name cannot be replaced by a file.
    std::fs::create_dir(cache_path(&s)).unwrap();
    assert!(write_atomic(&s, &env(2)).is_err());
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 2, "no temporary");
}

#[test]
fn a_stale_sidecar_is_none_and_keeps_the_threshold_it_carried() {
    let d = dir("stale");
    let s = source(&d);
    assert_eq!(check_read(&s).unwrap().env, None, "missing");
    std::fs::write(cache_path(&s), "not json").unwrap();
    assert_eq!(check_read(&s).unwrap().env, None);
    std::fs::write(cache_path(&s), [0xff, 0xfe, 0x00]).unwrap();
    assert_eq!(check_read(&s).unwrap().env, None, "not UTF-8");
    std::fs::write(
        cache_path(&s),
        r#"{"v":3,"fingerprint":"x","peak":[],"rms":[],"sensitivity":3.5}"#,
    )
    .unwrap();
    let c = check_read(&s).unwrap();
    assert_eq!((c.env, c.stored), (None, Some(3.5)), "too old");
    std::fs::write(cache_path(&s), format!("{}[", "[".repeat(100_000))).unwrap();
    assert_eq!(check_read(&s).unwrap().env, None, "nested 100,000 deep");
}

#[cfg(unix)]
#[test]
fn an_unreadable_sidecar_is_an_error_not_a_stale_one() {
    use std::os::unix::fs::PermissionsExt;
    let d = dir("denied");
    let s = source(&d);
    write_atomic(&s, &env(2)).unwrap();
    std::fs::set_permissions(cache_path(&s), std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(cache_path(&s)).is_err() {
        assert!(check_read(&s).is_err());
        assert!(read_checked(&s).is_err());
        assert!(k7_mediatek::sidecar::fresh_checked(&s, 2.0).is_err());
    }
    std::fs::set_permissions(cache_path(&s), std::fs::Permissions::from_mode(0o644)).unwrap();
    // A directory at the name is no sidecar and no stale one either: the write that follows would fail too.
    std::fs::remove_file(cache_path(&s)).unwrap();
    std::fs::create_dir(cache_path(&s)).unwrap();
    assert!(check_read(&s).is_err());
}

#[test]
fn a_sidecar_of_four_mib_is_answered_in_one_parse() {
    let d = dir("big");
    let s = source(&d);
    let small = serialise(&env(1));
    let at = small.find("\"peak\":[0.5000]").expect("the peak array");
    let (head, tail) = small.split_at(at);
    let tail = &tail["\"peak\":[0.5000]".len()..];
    let n = 4 * 1024 * 1024 / 7;
    let mut text = String::with_capacity(4 * 1024 * 1024 + 1024);
    text.push_str(head);
    text.push_str("\"peak\":[0.5000");
    text.push_str(&",0.5000".repeat(n - 1));
    text.push(']');
    text.push_str(tail);
    assert!(text.len() >= 4 * 1024 * 1024 - 8);
    std::fs::write(cache_path(&s), &text).unwrap();
    let t = std::time::Instant::now();
    let got = check_read(&s).unwrap();
    // A hang backstop, not a measure.
    let limit = if cfg!(target_env = "musl") { 60 } else { 15 };
    assert!(t.elapsed().as_secs() < limit, "{:?}", t.elapsed());
    assert_eq!(got.env.expect("answered").peak.len(), n);
    assert_eq!(got.stored, Some(2.0));
}
