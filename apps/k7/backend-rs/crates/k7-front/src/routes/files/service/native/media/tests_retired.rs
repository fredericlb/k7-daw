//! A prewarm retired during its plan starts no decode (review fix 1, M2): a locator whose project waits on a gate stands
//! for the plan, a cancel lands while it waits. What a reader gets wrong: the order is forced by the gate, never by a sleep.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::routes::files::service::native::callbacks::BoxFuture;
use serde_json::json;
use song::clips::ClipDoc;
use tokio::sync::Notify;

use super::prewarm::Status;
use super::testkit::{scratch, wav};
use super::waveform::{Located, Locator, ProjectView, Request as Asked, Why};
use super::{Config, Media};

fn clip(id: &str, source: &Path) -> ClipDoc {
    serde_json::from_value(json!({
        "id": id, "type": "audio", "name": id, "source": source.display().to_string(), "start": 0, "length": 4,
        "offset": 0, "fadeIn": 0, "fadeOut": 0, "loopStart": 0, "loopEnd": 0, "notes": []
    }))
    .unwrap()
}

/// Two stale clips, shown only once the gate opens.
pub(in crate::routes::files::service::native) struct Gated {
    pub dir: PathBuf,
    pub gate: Arc<Notify>,
}

impl Locator for Gated {
    fn locate(&self, _: Asked) -> BoxFuture<Result<Located, Why>> {
        Box::pin(async { Err(Why::Decline("test")) })
    }

    fn project(&self) -> BoxFuture<Result<ProjectView, Why>> {
        let (dir, gate) = (self.dir.clone(), self.gate.clone());
        Box::pin(async move {
            gate.notified().await;
            Ok(ProjectView {
                project: dir.display().to_string(),
                clips: (0..2)
                    .map(|i| clip(&format!("c{i}"), &dir.join(format!("c{i}.wav"))))
                    .collect(),
                roots: vec![dir.display().to_string()],
                grid: (120.0, 4.0, 4.0),
            })
        })
    }
}

#[tokio::test]
async fn a_cancel_that_lands_during_the_plan_leaves_the_run_with_no_decode() {
    let dir = scratch("retired_plan");
    for i in 0..2 {
        wav(&dir.join(format!("c{i}.wav")), 1.0, 3.0 + f64::from(i));
    }
    let env = [(
        "STUDIO_HOME".to_string(),
        crate::routes::files::service::native::tests::slashed(&dir.join("home")),
    )]
    .into();
    let media = Arc::new(Media::new(Config::of(&env, &dir), Arc::new(|| ())));
    let ws = dir.display().to_string();
    let gate = Arc::new(Notify::new());
    let locator = Arc::new(Gated {
        dir: dir.clone(),
        gate: gate.clone(),
    });
    let run = media.prewarms.begin(&ws);
    let planning = {
        let (m, ws) = (media.clone(), ws.clone());
        tokio::spawn(async move {
            m.prewarms
                .run(&ws, run, m.waves.clone(), locator, m.counts.clone())
                .await
        })
    };
    // The cancel finds no flight and answers at once, while the plan still waits.
    media.prewarms.cancel(&ws, &media.waves).await;
    gate.notify_one();
    let status = planning.await.unwrap();
    assert_eq!(status, Status::default());
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(media.counts.decodes.load(Ordering::Relaxed), 0);
    assert!(!media.envelopes.flying());
    assert_eq!(media.prewarms.status(&ws), Status::default());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_thousand_prewarms_and_cancels_interleaved_leave_no_decode_after_the_last() {
    let dir = scratch("retired_many");
    for i in 0..2 {
        wav(&dir.join(format!("c{i}.wav")), 1.0, 3.0 + f64::from(i));
    }
    let env = [(
        "STUDIO_HOME".to_string(),
        crate::routes::files::service::native::tests::slashed(&dir.join("home")),
    )]
    .into();
    let media = Arc::new(Media::new(Config::of(&env, &dir), Arc::new(|| ())));
    let ws = dir.display().to_string();
    let open = Arc::new(Notify::new());
    let mut runs = Vec::new();
    for i in 0..1000 {
        let locator = Arc::new(Gated {
            dir: dir.clone(),
            gate: open.clone(),
        });
        let run = media.prewarms.begin(&ws);
        let (m, w) = (media.clone(), ws.clone());
        runs.push(tokio::spawn(async move {
            m.prewarms
                .run(&w, run, m.waves.clone(), locator, m.counts.clone())
                .await
        }));
        if i % 2 == 1 {
            media.prewarms.cancel(&ws, &media.waves).await;
        }
    }
    media.prewarms.cancel(&ws, &media.waves).await;
    // A Notify keeps one permit, so each run is let through by waking until it is done.
    for r in runs {
        let end = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while !r.is_finished() {
            assert!(std::time::Instant::now() < end, "a run never left its plan");
            open.notify_waiters();
            open.notify_one();
            tokio::task::yield_now().await;
        }
        r.await.unwrap();
    }
    crate::routes::files::service::native::tests::until("no decode flies", || {
        !media.envelopes.flying() && media.envelopes.stopping() == 0
    })
    .await;
    assert_eq!(media.prewarms.status(&ws), Status::default());
    assert_eq!(media.counts.decodes.load(Ordering::Relaxed), 0);
}
