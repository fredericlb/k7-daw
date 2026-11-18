//! The envelopes' memo, their one decode per file, and the sidecar's rules (2410 to 2417), against the real waveform module
//! and a shell script standing in for the engine. What a reader gets wrong: `HOLD` keeps a script's decode open long enough to join.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
#[cfg(unix)]
use std::time::Duration;

use k7_mediatek::sidecar::cache_path;

#[cfg(unix)]
use super::envelopes::Stale;
#[cfg(unix)]
use super::flights;
#[cfg(unix)]
use super::testkit::{engine, overlap, starts};
use super::testkit::{envelopes, scratch, settings, turned, wav};

fn clip(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    wav(&p, 1.0, 3.0);
    p
}

#[cfg(unix)]
fn log_env(dir: &Path) -> Vec<(String, String)> {
    vec![("LOG".into(), dir.join("log").display().to_string())]
}

#[cfg(unix)]
fn locked_out(path: &Path, mode: u32) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
    let probe = path.join(".probe");
    let blocked = std::fs::File::create(&probe).is_err();
    let _ = std::fs::remove_file(&probe);
    blocked
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cold_clip_is_decoded_once_then_answered_from_the_memo() {
    let d = scratch("cold");
    let a = clip(&d, "a.wav");
    let (e, counts) = envelopes(settings(None, &[]));
    let first = e.ensure(&a, None).await.unwrap();
    assert!(cache_path(&a).exists() && first.peak.len() > 1);
    let again = e.ensure(&a, None).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again), "the memo answers");
    assert_eq!(counts.decodes.load(Ordering::Relaxed), 1);
    // A threshold asked that differs is a new decode, at that threshold.
    let tuned = e.ensure(&a, Some(3.0)).await.unwrap();
    assert_eq!(tuned.sensitivity, 3.0);
    assert_eq!(counts.decodes.load(Ordering::Relaxed), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_missing_source_is_nodes_enoent() {
    let d = scratch("enoent");
    let (e, _) = envelopes(settings(None, &[]));
    let err = e.ensure(&d.join("no.wav"), None).await.unwrap_err();
    assert!(
        err.starts_with("ENOENT: no such file or directory, stat '"),
        "{err}"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_link_at_the_sidecars_name_is_replaced_and_never_written_through() {
    let d = scratch("link");
    let a = clip(&d, "a.wav");
    let outside = d.join("outside.txt");
    std::fs::write(&outside, "not mine to write").unwrap();
    std::os::unix::fs::symlink(&outside, cache_path(&a)).unwrap();
    let (e, counts) = envelopes(settings(None, &[]));
    e.ensure(&a, None).await.unwrap();
    assert_eq!(
        std::fs::read_to_string(&outside).unwrap(),
        "not mine to write"
    );
    assert!(
        !std::fs::symlink_metadata(cache_path(&a))
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(counts.decodes.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_sidecar_that_is_not_one_is_a_stale_one_and_is_replaced() {
    let d = scratch("junk");
    let (e, counts) = envelopes(settings(None, &[]));
    let junk: Vec<Vec<u8>> = vec![
        b"{\"v\":4,\"fingerprint\":\"x\",\"peak\":[0.1".to_vec(),
        b"not json".to_vec(),
        "[".repeat(100_000).into_bytes(),
        br#"{"v":3,"fingerprint":"x","peak":[],"rms":[]}"#.to_vec(),
        br#"{"v":4,"fingerprint":"x","peak":["a","b"],"rms":[]}"#.to_vec(),
    ];
    for (i, text) in junk.iter().enumerate() {
        let a = clip(&d, &format!("j{i}.wav"));
        std::fs::write(cache_path(&a), text).unwrap();
        let env = e.ensure(&a, None).await.unwrap();
        assert!(env.peak.iter().all(|p| p.is_finite()), "case {i}");
    }
    assert_eq!(counts.decodes.load(Ordering::Relaxed), junk.len() as u64);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_sidecar_write_fails_the_waveform_and_nothing_is_remembered() {
    let d = scratch("unwritable");
    let a = clip(&d, "a.wav");
    let (e, _) = envelopes(settings(None, &[]));
    if locked_out(&d, 0o555) {
        assert!(e.ensure(&a, None).await.is_err());
        assert_eq!(e.held(), (0, 0), "the memo is set after the rename");
        assert!(
            e.ensure(&a, None).await.is_err(),
            "and so the next ask decodes again"
        );
    }
    locked_out(&d, 0o755);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unreadable_sidecar_is_an_error_and_decodes_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let d = scratch("denied");
    let a = clip(&d, "a.wav");
    let (e, counts) = envelopes(settings(None, &[]));
    e.ensure(&a, None).await.unwrap();
    e.invalidate(None);
    std::fs::set_permissions(cache_path(&a), std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read(cache_path(&a)).is_err() {
        assert!(e.ensure(&a, None).await.is_err());
        assert_eq!(
            counts.decodes.load(Ordering::Relaxed),
            1,
            "no second decode"
        );
        let stale = e.stale(std::slice::from_ref(&a));
        assert_eq!(
            stale,
            Stale {
                sources: vec![],
                unreadable: vec![a.clone()]
            }
        );
    }
    std::fs::set_permissions(cache_path(&a), std::fs::Permissions::from_mode(0o644)).unwrap();
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sixty_four_asks_for_one_stale_clip_are_one_decode() {
    let d = scratch("burst");
    let a = clip(&d, "a.wav");
    let bin = engine(&d, 8000, "");
    let mut s = settings(Some(bin), &[("HOLD", "1")]);
    s.env.extend(log_env(&d));
    let (e, counts) = envelopes(s);
    let asks: Vec<_> = (0..64)
        .map(|_| {
            let (e, a) = (e.clone(), a.clone());
            tokio::spawn(async move { e.ensure(&a, None).await })
        })
        .collect();
    let mut got = Vec::new();
    for t in asks {
        got.push(t.await.unwrap().unwrap());
    }
    assert_eq!(starts(&d.join("log")), 1);
    assert_eq!(counts.decodes.load(Ordering::Relaxed), 1);
    assert!(got.iter().all(|g| std::sync::Arc::ptr_eq(g, &got[0])));
    assert_eq!(
        flights::running(&a.display().to_string()),
        0,
        "the flight left"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sixty_four_asks_for_one_fresh_clip_read_its_sidecar_once() {
    let d = scratch("fresh");
    let a = clip(&d, "a.wav");
    let (e, counts) = envelopes(settings(None, &[]));
    e.ensure(&a, None).await.unwrap();
    e.invalidate(None);
    let asks: Vec<_> = (0..64)
        .map(|_| {
            let (e, a) = (e.clone(), a.clone());
            tokio::spawn(async move { e.ensure(&a, None).await })
        })
        .collect();
    let mut got = Vec::new();
    for t in asks {
        got.push(t.await.unwrap().unwrap());
    }
    assert_eq!(counts.decodes.load(Ordering::Relaxed), 1);
    assert!(
        got.iter().all(|g| std::sync::Arc::ptr_eq(g, &got[0])),
        "one parse answered every ask"
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_prewarms_cancel_stops_a_decode_nobody_waits_on_and_leaves_a_joined_one() {
    let d = scratch("cancel");
    let (a, b) = (clip(&d, "a.wav"), clip(&d, "b.wav"));
    let bin = engine(&d, 8000, "echo $$ >> \"$PIDS\"");
    let mut s = settings(Some(bin), &[("HOLD", "1")]);
    s.env
        .push(("LOG".into(), d.join("log").display().to_string()));
    s.env
        .push(("PIDS".into(), d.join("pids").display().to_string()));
    let (e, _) = envelopes(s);
    // a: a request and the prewarm share one decode; b: the prewarm alone.
    let req = {
        let (e, a) = (e.clone(), a.clone());
        tokio::spawn(async move { e.ensure(&a, None).await })
    };
    let warm_a = {
        let (e, a) = (e.clone(), a.clone());
        tokio::spawn(async move { e.warm(&a, "w").await })
    };
    let warm_b = {
        let (e, b) = (e.clone(), b.clone());
        tokio::spawn(async move { e.warm(&b, "w").await })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert_eq!(
        e.cancel_prewarm("w"),
        1,
        "only b's decode has no request on it"
    );
    assert!(warm_b.await.unwrap().unwrap_err().contains("was aborted"));
    assert!(req.await.unwrap().is_ok());
    assert!(
        warm_a.await.unwrap().is_ok(),
        "the joined decode ran to its end"
    );
    assert_eq!(flights::running(&a.display().to_string()), 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_engine_that_never_answers_is_killed_by_the_cancel_and_no_process_is_left() {
    let d = scratch("silent");
    let a = clip(&d, "a.wav");
    let bin = d.join("silent.sh");
    std::fs::write(
        &bin,
        "#!/bin/sh\necho $$ > \"$PIDFILE\"\nexec /bin/sleep 600\n",
    )
    .unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let pidfile = d.join("pid");
    let (e, _) = envelopes(settings(
        Some(bin),
        &[("PIDFILE", &pidfile.display().to_string())],
    ));
    let warm = {
        let (e, a) = (e.clone(), a.clone());
        tokio::spawn(async move { e.warm(&a, "w").await })
    };
    for _ in 0..800 {
        if std::fs::read_to_string(&pidfile).is_ok_and(|t| !t.trim().is_empty()) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(e.cancel_prewarm("w"), 1);
    let err = tokio::time::timeout(Duration::from_secs(10), warm)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(err.contains("was aborted"), "{err}");
    let pid = std::fs::read_to_string(&pidfile).unwrap();
    let alive = std::process::Command::new("kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success();
    assert!(!alive, "the child is gone");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_decode_child_sees_the_accounts_environment_and_nothing_of_the_fronts() {
    let d = scratch("env");
    let a = clip(&d, "a.wav");
    let bin = engine(&d, 0, "/usr/bin/env >&2\nexit 3");
    let (e, _) = envelopes(settings(
        Some(bin),
        &[
            ("LOG", &d.join("log").display().to_string()),
            ("ACCOUNT_ONLY", "yes"),
        ],
    ));
    let err = e.ensure(&a, None).await.unwrap_err();
    assert!(err.contains("ACCOUNT_ONLY=yes"), "{err}");
    assert!(!err.contains("HOME=") && !err.contains("CARGO"), "{err}");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_engine_claiming_a_trillion_frames_is_refused_before_it_decodes() {
    let d = scratch("claim");
    let a = clip(&d, "a.wav");
    let bin = engine(&d, 1_000_000_000_000, "");
    let script = std::fs::read_to_string(&bin).unwrap().replace(
        "/usr/bin/head -c 4000000000000 /dev/zero",
        "/usr/bin/head -c 4096 /dev/zero",
    );
    std::fs::write(&bin, script).unwrap();
    let (e, _) = envelopes(settings(
        Some(bin),
        &[("LOG", &d.join("log").display().to_string())],
    ));
    let err = e.ensure(&a, None).await.unwrap_err();
    assert!(err.contains("too long to analyse"), "{err}");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_gate_runs_two_decodes_at_once_and_one_for_a_local_user() {
    for (local, most) in [(false, 2usize), (true, 1)] {
        let d = scratch(&format!("gate{local}"));
        let bin = engine(&d, 8000, "");
        let mut s = settings(Some(bin), &[("HOLD", "0.4")]);
        s.env
            .push(("LOG".into(), d.join("log").display().to_string()));
        s.local_user = local;
        let (e, _) = envelopes(s);
        let tasks: Vec<_> = (0..4)
            .map(|i| {
                let (e, a) = (e.clone(), clip(&d, &format!("{i}.wav")));
                tokio::spawn(async move { e.ensure(&a, None).await })
            })
            .collect();
        for t in tasks {
            t.await.unwrap().unwrap();
        }
        assert_eq!(overlap(&d.join("log")), most, "local user {local}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_memo_is_bounded_in_bytes_and_let_go_when_unused() {
    let d = scratch("memo");
    let (clock, by) = turned();
    let mut s = settings(None, &[]);
    s.clock = clock;
    let (probe, _) = envelopes(settings(None, &[]));
    let one = clip(&d, "w0.wav");
    let weight = probe.ensure(&one, None).await.unwrap().peak.len() * 8 * 2;
    s.memo_bytes = weight * 5 / 2;
    let (e, _) = envelopes(s);
    for i in 0..4 {
        let a = clip(&d, &format!("w{i}.wav"));
        e.ensure(&a, None).await.unwrap();
    }
    let (n, bytes) = e.held();
    assert!((1..4).contains(&n), "{n} held under the bound");
    assert!(bytes <= weight * 5 / 2 + weight, "{bytes} bytes");
    by.store(11 * 60, Ordering::Relaxed);
    e.reap();
    assert_eq!(e.held(), (0, 0), "let go after ten idle minutes");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_yes_of_freshness_is_remembered_and_a_no_never() {
    let d = scratch("stale");
    let (a, b) = (clip(&d, "a.wav"), clip(&d, "b.wav"));
    let (e, _) = envelopes(settings(None, &[]));
    e.ensure(&a, None).await.unwrap();
    let both = vec![a.clone(), b.clone(), d.join("gone.wav"), a.clone()];
    let first = e.stale(&both);
    assert_eq!(
        first.sources,
        vec![b.clone()],
        "a is fresh, gone drops, a repeat counts once"
    );
    assert_eq!(e.fresh_len(), 1);
    std::fs::remove_file(cache_path(&a)).unwrap();
    assert_eq!(e.stale(std::slice::from_ref(&a)).sources, vec![a.clone()]);
    assert_eq!(e.fresh_len(), 0, "a NO is never kept");
}
