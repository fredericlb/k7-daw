//! The apps' downloads the front owns, over a temp folder and a backend that is not there: the CI's bearer,
//! upload then publish per channel and per store, the feed, the page, ranges and validators, and the on-disk layout.

use std::net::SocketAddr;
use std::path::PathBuf;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const TOKEN: &str = "ci-secret";

struct Front {
    addr: SocketAddr,
    root: PathBuf,
}

async fn front(name: &str, extra: &[(&str, &str)]) -> Front {
    let root =
        std::env::temp_dir().join(format!("k7-front-releases-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let gone = TcpListener::bind("127.0.0.1:0")
        .await
        .unwrap()
        .local_addr()
        .unwrap();
    let mut env: std::collections::BTreeMap<String, String> = [
        ("STUDIO_APP_RELEASES_DIR", root.to_str().unwrap()),
        ("APP_RELEASE_TOKEN", TOKEN),
        ("APP_RELEASE_MAX_MB", "8"),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect();
    for (k, v) in extra {
        if v.is_empty() {
            env.remove(*k);
        } else {
            env.insert(k.to_string(), v.to_string());
        }
    }
    let config = k7_front::Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        upstream: gone.to_string(),
        secret: "s".into(),
        trust_hops: 0,
        env,
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(k7_front::serve(config, std::future::pending(), move |a| {
        tx.send(a).unwrap()
    }));
    Front {
        addr: rx.await.unwrap(),
        root,
    }
}

/// Status line, headers (lowercased) and the body as text.
struct Answer {
    head: String,
    body: Vec<u8>,
}

impl Answer {
    fn status(&self) -> u16 {
        self.head[9..12].parse().unwrap()
    }
    fn header(&self, name: &str) -> Option<String> {
        self.head.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.eq_ignore_ascii_case(name).then(|| v.trim().to_string())
        })
    }
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

async fn send(addr: SocketAddr, method: &str, path: &str, headers: &[&str], body: &[u8]) -> Answer {
    let mut s = TcpStream::connect(addr).await.unwrap();
    let host = if headers
        .iter()
        .any(|h| h.to_ascii_lowercase().starts_with("host:"))
    {
        ""
    } else {
        "Host: 127.0.0.1\r\n"
    };
    let mut r = format!(
        "{method} {path} HTTP/1.1\r\n{host}Connection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for h in headers {
        r.push_str(h);
        r.push_str("\r\n");
    }
    r.push_str("\r\n");
    s.write_all(r.as_bytes()).await.unwrap();
    // A refusal may close before the whole body is taken, as the backend's 413 does.
    let _ = s.write_all(body).await;
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out).await;
    let at = out
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a head")
        + 4;
    Answer {
        head: String::from_utf8_lossy(&out[..at]).into_owned(),
        body: out[at..].to_vec(),
    }
}

const AUTH: &str = "Authorization: Bearer ci-secret";

async fn build(f: &Front, under: &str, stamp: &str, exe: &[u8]) {
    let exe_name = format!("k7_{stamp}_windows-x64.exe");
    let put = send(
        f.addr,
        "PUT",
        &format!("/api/app-releases/{under}{stamp}/{exe_name}"),
        &[AUTH],
        exe,
    )
    .await;
    assert_eq!(put.status(), 201, "{}", put.text());
    assert_eq!(
        put.text(),
        format!("{{\"name\":\"{exe_name}\",\"bytes\":{}}}", exe.len())
    );
    let yml = send(
        f.addr,
        "PUT",
        &format!("/api/app-releases/{under}{stamp}/latest.yml"),
        &[AUTH],
        b"path: x",
    )
    .await;
    assert_eq!(yml.status(), 201);
    let publish = send(
        f.addr,
        "POST",
        &format!("/api/app-releases/{under}{stamp}/publish"),
        &[AUTH],
        b"",
    )
    .await;
    assert_eq!(publish.status(), 200, "{}", publish.text());
}

#[tokio::test]
async fn uploads_take_the_ci_bearer_only() {
    let f = front("bearer", &[]).await;
    let path = "/api/app-releases/v0.1.0/a.exe";
    assert_eq!(send(f.addr, "PUT", path, &[], b"x").await.status(), 401);
    let wrong = send(f.addr, "PUT", path, &["Authorization: Bearer nope"], b"x").await;
    assert_eq!(
        (wrong.status(), wrong.text()),
        (401, r#"{"error":"unauthorized"}"#.into())
    );
    assert_eq!(
        send(f.addr, "POST", "/api/app-releases/v0.1.0/publish", &[], b"")
            .await
            .status(),
        401
    );
    let bad = send(f.addr, "PUT", "/api/app-releases/v0.1/a.exe", &[AUTH], b"x").await;
    assert_eq!(
        (bad.status(), bad.text()),
        (400, r#"{"error":"invalid stamp or file"}"#.into())
    );
    assert_eq!(
        send(f.addr, "PUT", "/api/app-releases/v0.1.0/.a", &[AUTH], b"x")
            .await
            .status(),
        400
    );
    assert_eq!(
        send(f.addr, "PUT", "/api/app-releases/tray/a.exe", &[AUTH], b"x")
            .await
            .status(),
        400
    );
    let none = send(
        f.addr,
        "POST",
        "/api/app-releases/v9.9.9/publish",
        &[AUTH],
        b"",
    )
    .await;
    assert_eq!(
        (none.status(), none.text()),
        (404, r#"{"error":"nothing uploaded for v9.9.9"}"#.into())
    );
    let unset = front("unset", &[("APP_RELEASE_TOKEN", "")]).await;
    assert_eq!(
        send(unset.addr, "PUT", path, &[AUTH], b"x").await.status(),
        401
    );
}

#[tokio::test]
async fn a_published_build_is_fed_paged_and_served_by_ranges() {
    let f = front("publish", &[]).await;
    let exe: Vec<u8> = (0..=255u8).cycle().take(100_000).collect();
    build(&f, "", "v0.1.0", &exe).await;
    assert_eq!(
        std::fs::read_to_string(f.root.join("release/current")).unwrap(),
        "v0.1.0"
    );
    assert!(f.root.join("release/v0.1.0/latest.yml").is_file());
    assert!(!f.root.join("release/incoming/v0.1.0").exists());

    let feed = send(f.addr, "GET", "/app_downloads/release.json", &[], b"").await;
    assert_eq!(feed.status(), 200);
    assert_eq!(feed.header("cache-control").as_deref(), Some("no-cache"));
    let json: serde_json::Value = serde_json::from_slice(&feed.body).unwrap();
    assert_eq!(json["channel"], "release");
    assert_eq!(json["stamp"], "v0.1.0");
    let keys: Vec<&String> = json.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["channel", "stamp", "publishedAt", "files"]);
    assert_eq!(json["files"][0]["name"], "k7_v0.1.0_windows-x64.exe");
    assert_eq!(
        json["files"][1],
        serde_json::json!({ "name": "latest.yml", "bytes": 7 })
    );
    assert_eq!(
        send(f.addr, "GET", "/app_downloads/snapshot.json", &[], b"")
            .await
            .status(),
        404
    );
    assert_eq!(
        send(f.addr, "GET", "/app_downloads/tray/release.json", &[], b"")
            .await
            .status(),
        404
    );

    let url = "/app_downloads/release/k7_v0.1.0_windows-x64.exe";
    let whole = send(f.addr, "GET", url, &[], b"").await;
    assert_eq!(whole.status(), 200);
    assert_eq!(whole.body, exe);
    assert_eq!(
        whole.header("content-disposition").as_deref(),
        Some("attachment")
    );
    assert_eq!(
        whole.header("content-type").as_deref(),
        Some("application/octet-stream")
    );
    assert_eq!(whole.header("accept-ranges").as_deref(), Some("bytes"));
    let etag = whole.header("etag").unwrap();
    assert!(etag.starts_with("W/\"186a0-"), "{etag}");

    let part = send(f.addr, "GET", url, &["Range: bytes=2-5"], b"").await;
    assert_eq!(part.status(), 206);
    assert_eq!(
        part.header("content-range").as_deref(),
        Some("bytes 2-5/100000")
    );
    assert_eq!(part.body, exe[2..6]);
    let tail = send(f.addr, "GET", url, &["Range: bytes=-3"], b"").await;
    assert_eq!(tail.body, exe[99_997..]);
    let past = send(f.addr, "GET", url, &["Range: bytes=100000-"], b"").await;
    assert_eq!(past.status(), 416);
    assert_eq!(
        past.header("content-range").as_deref(),
        Some("bytes */100000")
    );
    let held = send(
        f.addr,
        "GET",
        url,
        &[&format!("If-None-Match: {etag}")],
        b"",
    )
    .await;
    assert_eq!((held.status(), held.body.len()), (304, 0));
    let stale = send(
        f.addr,
        "GET",
        url,
        &["Range: bytes=2-5", "If-Range: \"other\""],
        b"",
    )
    .await;
    assert_eq!((stale.status(), stale.body.len()), (200, 100_000));
    let head = send(f.addr, "HEAD", url, &[], b"").await;
    assert_eq!(
        (head.status(), head.header("content-length")),
        (200, Some("100000".into()))
    );
    let yml = send(f.addr, "GET", "/app_downloads/release/latest.yml", &[], b"").await;
    assert_eq!(yml.header("content-type").as_deref(), Some("text/plain"));
    assert_eq!(yml.header("content-disposition"), None);
    let missing = send(f.addr, "GET", "/app_downloads/release/nope.exe", &[], b"").await;
    assert_eq!(
        (missing.status(), missing.text()),
        (404, "not found".into())
    );

    let page = send(f.addr, "GET", "/app_downloads", &[], b"").await;
    assert_eq!(
        page.header("content-type").as_deref(),
        Some("text/html; charset=utf-8")
    );
    assert!(
        page.text().contains("<h2>Release 0.1.0</h2>"),
        "{}",
        page.text()
    );
    assert!(
        page.text()
            .contains(r#"<a href="app_downloads/release/k7_v0.1.0_windows-x64.exe">Windows</a>"#)
    );
    let fr = send(
        f.addr,
        "GET",
        "/app_downloads",
        &["Accept-Language: fr-FR,fr;q=0.9"],
        b"",
    )
    .await;
    assert!(fr.text().contains("k7 — téléchargements"));
    let slash = send(f.addr, "GET", "/app_downloads/", &[], b"").await;
    assert_eq!(
        (slash.status(), slash.header("location")),
        (301, Some("/app_downloads".into()))
    );
}

#[tokio::test]
async fn a_channel_keeps_its_previous_build_and_the_tray_its_own() {
    let f = front("previous", &[]).await;
    build(&f, "", "snapshot-aaaaaaa", b"one").await;
    build(&f, "", "snapshot-bbbbbbb", b"two").await;
    build(&f, "", "snapshot-ccccccc", b"three").await;
    let dir = f.root.join("snapshot");
    assert_eq!(
        std::fs::read_to_string(dir.join("previous")).unwrap(),
        "snapshot-bbbbbbb"
    );
    assert!(
        !dir.join("snapshot-aaaaaaa").exists(),
        "only current and previous stay"
    );
    let old = send(
        f.addr,
        "GET",
        "/app_downloads/snapshot/k7_snapshot-bbbbbbb_windows-x64.exe",
        &[],
        b"",
    )
    .await;
    assert_eq!((old.status(), old.text()), (200, "two".into()));
    build(&f, "tray/", "v1.0.0", b"tray").await;
    assert!(f.root.join("tray/release/v1.0.0").is_dir());
    let tray = send(
        f.addr,
        "GET",
        "/app_downloads/tray/release/k7_v1.0.0_windows-x64.exe",
        &[],
        b"",
    )
    .await;
    assert_eq!(tray.text(), "tray");
    assert_eq!(
        send(f.addr, "GET", "/app_downloads/release.json", &[], b"")
            .await
            .status(),
        404
    );
    let page = send(f.addr, "GET", "/app_downloads", &[], b"").await.text();
    assert!(
        page.contains("<h2>Tray — Release 1.0.0</h2>")
            && page.contains("<h2>Latest build of main</h2>")
    );
}

#[tokio::test]
async fn an_upload_past_the_ceiling_is_refused_and_leaves_nothing() {
    let f = front("large", &[("APP_RELEASE_MAX_MB", "1")]).await;
    let big = vec![7u8; 1024 * 1024 + 1];
    let refused = send(
        f.addr,
        "PUT",
        "/api/app-releases/v0.2.0/big.zip",
        &[AUTH],
        &big,
    )
    .await;
    assert_eq!(refused.status(), 413);
    assert_eq!(refused.text(), r#"{"error":"file exceeds 1048576 bytes"}"#);
    let dir = f.root.join("release/incoming/v0.2.0");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "no .part left");
    let fits = send(
        f.addr,
        "PUT",
        "/api/app-releases/v0.2.0/big.zip",
        &[AUTH],
        &big[1..],
    )
    .await;
    assert_eq!(fits.status(), 201);
    assert_eq!(
        std::fs::metadata(dir.join("big.zip")).unwrap().len(),
        1024 * 1024
    );
}

#[tokio::test]
async fn the_family_answers_the_root_domain_and_is_the_backends_on_a_device() {
    let f = front("hosts", &[]).await;
    let site = send(f.addr, "GET", "/app_downloads", &["Host: k7.rocks"], b"").await;
    assert_eq!(site.status(), 200, "{}", site.head);
    let other = send(f.addr, "GET", "/api/app-releases/v0.1.0/a.exe", &[], b"").await;
    assert_eq!(
        other.status(),
        502,
        "a read of the upload path is the backend's"
    );
    let device = front("device", &[("STUDIO_LOCAL_USER", "1")]).await;
    let out = send(device.addr, "GET", "/app_downloads", &[], b"").await;
    assert_eq!(out.status(), 502, "not mounted on a device: {}", out.head);
}
