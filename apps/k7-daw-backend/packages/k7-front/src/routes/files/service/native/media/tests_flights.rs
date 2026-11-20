//! The decodes in flight against a stand-in engine: the prewarm and a request share one, and a request after a cancel is
//! answered. What a reader gets wrong: `HOLD` keeps the script's decode open long enough for the asks to meet.

use super::flights;
#[cfg(unix)]
use {
    super::testkit::{engine, envelopes, scratch, settings, starts, warm_module, wav},
    std::{path::Path, time::Duration},
};

#[cfg(unix)]
fn held(d: &Path) -> super::envelopes::Envelopes {
    let bin = engine(d, 8000, "");
    let log = d.join("log").display().to_string();
    let (e, _) = envelopes(settings(Some(bin), &[("HOLD", "1"), ("LOG", &log)]));
    e
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_prewarm_and_a_request_without_a_threshold_decode_once() {
    warm_module().await;
    let d = scratch("flight_one");
    let a = d.join("a.wav");
    wav(&a, 1.0, 3.0);
    let e = held(&d);
    let warm = {
        let (e, a) = (e.clone(), a.clone());
        tokio::spawn(async move { e.warm(&a, "w").await })
    };
    for _ in 0..400 {
        if starts(&d.join("log")) == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let asked = e.ensure(&a, None).await;
    assert!(asked.is_ok() && warm.await.unwrap().is_ok());
    assert_eq!(starts(&d.join("log")), 1, "one decode for both");
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_request_after_a_prewarms_cancel_is_answered_not_aborted() {
    warm_module().await;
    let d = scratch("flight_cancel");
    let a = d.join("a.wav");
    wav(&a, 1.0, 3.0);
    let e = held(&d);
    let warm = {
        let (e, a) = (e.clone(), a.clone());
        tokio::spawn(async move { e.warm(&a, "w").await })
    };
    for _ in 0..400 {
        if starts(&d.join("log")) == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(e.cancel_prewarm("w"), 1);
    let asked = e.ensure(&a, None).await;
    assert!(asked.is_ok(), "{asked:?}");
    assert!(warm.await.unwrap().unwrap_err().contains("was aborted"));
    assert_eq!(starts(&d.join("log")), 2, "the request ran its own decode");
    assert_eq!(flights::running(&a.display().to_string()), 0);
}

#[test]
fn a_stopped_flight_leaving_does_not_drop_the_one_that_replaced_it() {
    let key = flights::key("/flights/own-entry.wav", None);
    let old = flights::join(&key, false, 7, "w");
    old.flight
        .stop
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let new = flights::join(&key, true, 7, "w");
    assert!(new.starts.is_some(), "a stopped flight is replaced");
    flights::leave(&key, &old.flight);
    assert_eq!(flights::running("/flights/own-entry.wav"), 1);
    flights::leave(&key, &new.flight);
    assert_eq!(flights::running("/flights/own-entry.wav"), 0);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_cancel_stops_its_own_workspaces_decodes_and_never_another_workspaces() {
    let d = scratch("flight_ws");
    let (a, b) = (d.join("a.wav"), d.join("b.wav"));
    wav(&a, 1.0, 3.0);
    wav(&b, 1.0, 4.0);
    let e = held(&d);
    let spawn = |p: &Path, ws: &'static str| {
        let (e, p) = (e.clone(), p.to_path_buf());
        tokio::spawn(async move { e.warm(&p, ws).await })
    };
    let (wa, wb) = (spawn(&a, "A"), spawn(&b, "B"));
    // A file two workspaces prewarm is stopped only when both have cancelled.
    let (wa2, wb2) = (spawn(&a, "B"), spawn(&b, "A"));
    for _ in 0..400 {
        if starts(&d.join("log")) == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(e.cancel_prewarm("A"), 0, "B still wants both files");
    assert_eq!(e.cancel_prewarm("B"), 2, "nobody wants them now");
    for w in [wa, wb, wa2, wb2] {
        let _ = tokio::time::timeout(Duration::from_secs(20), w).await;
    }
}
