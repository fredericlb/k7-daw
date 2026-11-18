//! Scenarios of the engine link's frame replays: a take filed, a browser's document and the reload that outranks it, the
//! project recycled, the binding told again, a copy that fails and is retried. Helpers are `tests_scene`'.

use std::time::Duration;

use serde_json::json;

use super::fake::FakeJobs;
use super::tests_real::Account;
use super::tests_scene::{frames, is_frame, open, playable, until_frame, verb};

pub(super) fn silent_wav(path: &std::path::Path) {
    let mut b = vec![0u8; 44];
    b[..4].copy_from_slice(b"RIFF");
    b[4..8].copy_from_slice(&36u32.to_le_bytes());
    b[8..16].copy_from_slice(b"WAVEfmt ");
    b[16..20].copy_from_slice(&16u32.to_le_bytes());
    b[20..22].copy_from_slice(&1u16.to_le_bytes());
    b[22..24].copy_from_slice(&2u16.to_le_bytes());
    b[24..28].copy_from_slice(&48000u32.to_le_bytes());
    b[28..32].copy_from_slice(&192000u32.to_le_bytes());
    b[32..34].copy_from_slice(&4u16.to_le_bytes());
    b[34..36].copy_from_slice(&16u16.to_le_bytes());
    b[36..40].copy_from_slice(b"data");
    std::fs::write(path, b).unwrap();
}

/// `POST /engine/project`'s act: the front's own route handler.
pub(super) fn client_doc(a: &Account, doc: serde_json::Value) {
    let e = a.engine.as_ref().expect("the front's side");
    e.link.session(Some(&a.ws)).apply_client_now(doc);
}

/// `POST /engine/sync/retry`'s act: the failed copy forgotten and the project sent again.
pub(super) async fn retry(a: &Account) {
    let e = a.engine.as_ref().expect("the front's side");
    e.link
        .session(Some(&a.ws))
        .retry_sync(&a.ws, &format!("{}/p.k7", a.ws))
        .await;
}

/// An armed lane reports a take: it is filed into the lane and the project is sent with the new clip.
pub(super) fn take_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let seen = frames(a).len();
        verb(a, "transport arm gtr on").await;
        until_frame(a, seen, "the arm", is_frame("arm")).await;
        let wav = std::path::Path::new(&a.ws).join("p.k7/clips/take-tmp.wav");
        silent_wav(&wav);
        let seen = frames(a).len();
        a.fake.send_all(json!({
            "type": "recorded", "track": "gtr", "file": wav.display().to_string(), "start": 0, "length": 2,
        }));
        until_frame(a, seen, "the document with the take", |f| {
            f["type"] == "project"
                && f["doc"]["tracks"][0]["clips"]
                    .as_array()
                    .is_some_and(|c| !c.is_empty())
        })
        .await;
    })
}

/// A browser's document, then the reload that outranks it.
pub(super) fn client_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let seen = frames(a).len();
        let mut doc = super::fake::minimal_doc(-5.0);
        doc["name"] = json!("browser");
        client_doc(a, doc);
        until_frame(a, seen, "the browser's document", |f| {
            f["type"] == "project" && f["doc"]["name"] == json!("browser")
        })
        .await;
        let seen = frames(a).len();
        retry(a).await;
        until_frame(a, seen, "the reload's document", |f| {
            f["type"] == "project" && f["doc"]["name"] != json!("browser")
        })
        .await;
    })
}

/// The open project recycled under the engine: the session is let go.
pub(super) fn recycle_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let seen = frames(a).len();
        let (mut p, _) =
            crate::routes::files::service::native::tests_projects::proj(&a.dir, "recycle");
        p.engine = a.engine.clone();
        let (status, body) = crate::routes::files::service::native::tests_projects::go(
            &p,
            crate::routes::files::service::native::projects::Kind::Recycle,
            &a.dir,
            json!({ "path": "p.k7" }),
        )
        .await;
        assert_eq!(status, 200, "{body}");
        until_frame(a, seen, "the drop", is_frame("drop-session")).await;
    })
}

/// The binding told again (nothing moves), cleared (the session is let go), and restored (the project is sent again).
pub(super) fn binding_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        open(a, name, "p.k7").await;
        let bound = json!({ "url": a.fake.url, "engineId": "e1" });
        a.bind(&bound);
        // The same URL again closes nothing: the next verb's frame reaches the same socket.
        let (seen, sockets) = (frames(a).len(), a.fake.accepted());
        verb(a, "transport arm gtr on").await;
        until_frame(a, seen, "the arm after a rebind", is_frame("arm")).await;
        assert_eq!(a.fake.accepted(), sockets, "{name}: a second socket");
        a.bind(&json!({ "url": null }));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let seen = frames(a).len();
        a.bind(&bound);
        a.start().await;
        until_frame(a, seen, "the project again", is_frame("project")).await;
        playable(
            a,
            name,
            std::time::Instant::now(),
            &format!("{}/p.k7", a.ws),
        )
        .await;
    })
}

/// A copy that fails holds the document; the retry sends it once the jobs agent answers.
pub(super) fn copy_scenario<'a>(
    a: &'a Account,
    name: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>> {
    Box::pin(async move {
        let jobs = FakeJobs::start().await;
        jobs.ends(super::fake::JobEnd::Synced {
            failed: 1,
            failures: vec!["a.wav: gone".into()],
        });
        let p = std::path::Path::new(&a.ws).join("p.k7");
        silent_wav(&p.join("clips/a.wav"));
        std::fs::write(
            p.join("clips/a1.k7x"),
            "source(\"clips/a.wav\")\nfrom(\"1:1\")\nto(\"2:1\")\n",
        )
        .unwrap();
        std::fs::write(
            p.join("nodes/gtr.k7x"),
            "extends(\"@meta/track\")\nevents([{ ref: \"a1\", start: \"1:1\" }])\n",
        )
        .unwrap();
        a.bind(&json!({ "url": a.fake.url, "jobsUrl": jobs.url, "engineId": "e1" }));
        a.open("p.k7");
        a.start().await;
        let end = std::time::Instant::now() + Duration::from_secs(20);
        while jobs.asked().is_empty() {
            assert!(std::time::Instant::now() < end, "the copy was never asked");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            !frames(a).iter().any(|f| f["type"] == "project"),
            "a document went before its files"
        );
        jobs.ends(super::fake::JobEnd::Synced {
            failed: 0,
            failures: vec![],
        });
        retry(a).await;
        until_frame(a, 0, "the document after the retry", is_frame("project")).await;
        playable(
            a,
            name,
            std::time::Instant::now(),
            &format!("{}/p.k7", a.ws),
        )
        .await;
    })
}
