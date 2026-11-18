//! tests.rs — the request gate's decision, and the browser held to it when a real chromium is named (`K7_LIVE_CHROMIUM`).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

/// 2506: the job's own path passes; every other address, the loopback front and the metadata address included, fails.
#[test]
fn the_gate_lets_only_the_jobs_own_path_through() {
    let ok = |u: &str| allowed(u, 4000, "tok");
    assert!(ok("http://127.0.0.1:4000/tok/job.json"));
    assert!(ok("http://127.0.0.1:4000/tok/media/a%2Fb.mp4"));
    assert!(ok("data:text/plain;base64,AA=="));
    assert!(ok("blob:http://127.0.0.1:4000/x"));
    assert!(ok("about:blank"));
    assert!(!ok("http://169.254.169.254/latest/meta-data/"));
    assert!(!ok("http://127.0.0.1:18789/api/me"));
    assert!(!ok("http://127.0.0.1:4000/other/job.json"));
    assert!(!ok("http://127.0.0.1:40001/tok/job.json"));
    assert!(!ok("http://127.0.0.1:4000@evil.example/tok/"));
    assert!(!ok("file:///etc/passwd"));
    assert!(!ok("https://127.0.0.1:4000/tok/"));
}

/// A page that asks for the addresses its job lists, then posts how many it reached.
const PAGE: &str = r#"
const job = await (await fetch("job.json")).json();
let reached = 0;
for (const u of job.urls) {
  try { await fetch(u, { mode: "no-cors", signal: AbortSignal.timeout(3000) }); reached++; } catch {}
}
await fetch("result?type=video%2Fwebm&last=1", { method: "POST", headers: { "content-type": "text/plain" }, body: btoa("reached=" + reached) });
"#;

/// With a real chromium (`K7_LIVE_CHROMIUM`), a page naming the loopback front and the metadata address reaches neither (2506).
#[tokio::test]
async fn with_a_live_chromium_the_browser_fetches_nothing_but_its_job() {
    let Some(chromium) = std::env::var_os("K7_LIVE_CHROMIUM") else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("k7-picture-gate-{}", token()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("page.js");
    std::fs::write(&script, format!("(async () => {{{PAGE}}})();")).unwrap();
    // The front's stand-in: counts every connection the browser makes to it.
    let front = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let front_port = front.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let counted = hits.clone();
    let listener = tokio::spawn(async move {
        while front.accept().await.is_ok() {
            counted.fetch_add(1, Ordering::SeqCst);
        }
    });
    let urls = [
        format!("http://127.0.0.1:{front_port}/api/me"),
        "http://169.254.169.254/latest/meta-data/".to_string(),
    ];
    let job = format!("{{\"urls\":{}}}", serde_json::to_string(&urls).unwrap());
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = lines.clone();
    let env: Vec<(String, String)> = ["PATH", "TMPDIR"]
        .iter()
        .filter_map(|k| Some((k.to_string(), std::env::var(k).ok()?)))
        .collect();
    let r = render(Job {
        chromium: PathBuf::from(chromium),
        script,
        job,
        mix: None,
        media: HashMap::new(),
        out: dir.join("film"),
        debug: false,
        env,
        log: Arc::new(move |l| sink.lock().unwrap().push(l.to_string())),
    })
    .await
    .unwrap();
    listener.abort();
    assert_eq!(
        std::fs::read_to_string(&r.part).unwrap(),
        "reached=0",
        "{:?}",
        lines.lock().unwrap()
    );
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "the browser reached the front"
    );
    assert!(r.blocked.iter().any(|u| u == &urls[0]), "{:?}", r.blocked);
    assert!(r.blocked.iter().any(|u| u == &urls[1]), "{:?}", r.blocked);
    drop(r);
    let _ = std::fs::remove_dir_all(&dir);
}
