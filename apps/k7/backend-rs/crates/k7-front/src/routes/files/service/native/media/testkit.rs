//! testkit.rs — what the media pieces' tests share: a scratch folder, a WAV, a shell script standing in for the engine, a
//! raw HTTP server on `127.0.0.1:0`, and a clock a test turns. What a reader gets wrong: every path here is the test's own.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use super::Clock;
use super::envelopes::{Envelopes, MEMO_BYTES, Settings};

/// An empty folder of this test's own, under the temp dir.
pub fn scratch(name: &str) -> PathBuf {
    let d = crate::routes::files::service::native::tests::tmp_root()
        .join(format!("k7-media-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The waveform module compiled before a test's waits begin: a debug build takes longer than they allow, once per process.
pub async fn warm_module() {
    let made = tokio::task::spawn_blocking(|| super::direct::module(None).map(drop)).await;
    made.unwrap().unwrap();
}

/// A mono 16-bit WAV of a tone, `seconds` long at 8 kHz.
pub fn wav(path: &Path, seconds: f64, tone: f64) {
    let n = (seconds * 8000.0) as usize;
    let mut b = Vec::with_capacity(44 + n * 2);
    let data = (n * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    for v in [16u32, 0x0001_0001, 8000, 16000, 0x0010_0002] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for i in 0..n {
        let s = ((i as f64 * tone * 0.01).sin() * 12000.0) as i16;
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// A shell script that answers `--decode` with a header and `frames` silent frames, logging `s` at its start and `e` at its end
/// to `$LOG`, after `$HOLD` seconds. Commands are absolute: its environment is the account's alone.
#[cfg(unix)]
pub fn engine(dir: &Path, frames: u64, extra: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let bin = dir.join("engine.sh");
    let body = format!(
        "#!/bin/sh\necho s >> \"$LOG\"\n{extra}\n[ -n \"$HOLD\" ] && /bin/sleep \"$HOLD\"\nprintf '{{\"sampleRate\":8000,\"channels\":1,\"frames\":{frames}}}\\n'\n/usr/bin/head -c {} /dev/zero\necho e >> \"$LOG\"\n",
        frames * 4
    );
    std::fs::write(&bin, body).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

#[cfg(unix)]
/// The most runs of the engine that overlapped, read off its log of starts and ends.
pub fn overlap(log: &Path) -> usize {
    let (mut now, mut most) = (0usize, 0usize);
    for l in std::fs::read_to_string(log).unwrap_or_default().lines() {
        match l {
            "s" => now += 1,
            "e" => now = now.saturating_sub(1),
            _ => {}
        }
        most = most.max(now);
    }
    most
}

#[cfg(unix)]
/// How many times the engine started.
pub fn starts(log: &Path) -> usize {
    std::fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .filter(|l| *l == "s")
        .count()
}

/// A clock a test moves forward.
pub fn turned() -> (Clock, Arc<AtomicU64>) {
    let base = Instant::now();
    let by = Arc::new(AtomicU64::new(0));
    let b = by.clone();
    (
        Arc::new(move || base + Duration::from_secs(b.load(Ordering::Relaxed))),
        by,
    )
}

pub fn settings(engine: Option<PathBuf>, env: &[(&str, &str)]) -> Settings {
    Settings {
        engine,
        env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        wasm_cache: None,
        local_user: false,
        memo_bytes: MEMO_BYTES,
        idle: super::KEEP_IDLE,
        clock: Arc::new(Instant::now),
    }
}

pub fn envelopes(s: Settings) -> (Envelopes, Arc<super::Counts>) {
    let counts = Arc::new(super::Counts::default());
    (Envelopes::new(s, counts.clone()), counts)
}

/// What a raw server was asked.
#[derive(Clone, Debug)]
pub struct Seen {
    pub head: String,
}

/// What the server answers.
pub enum Reply {
    /// A status, its headers, its body, and the length it announces when that is not the body's.
    Is {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
        announce: Option<usize>,
    },
    /// Read the request and never answer.
    Hang,
    /// Announce `announce` bytes, send `body` and then say nothing more.
    Stall { announce: usize, body: Vec<u8> },
}

impl Reply {
    pub fn ok(body: &[u8]) -> Reply {
        Reply::Is {
            status: 200,
            headers: vec![],
            body: body.to_vec(),
            announce: None,
        }
    }

    pub fn with(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Reply {
        Reply::Is {
            status,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            body: body.to_vec(),
            announce: None,
        }
    }
}

type Handler = Arc<dyn Fn(&Seen) -> Reply + Send + Sync>;

/// A server on `127.0.0.1:0` answering each connection once; the address, and every request it saw.
pub async fn serve(
    handler: impl Fn(&Seen) -> Reply + Send + Sync + 'static,
) -> (String, Arc<Mutex<Vec<Seen>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = format!("http://{}", listener.local_addr().unwrap());
    let seen: Arc<Mutex<Vec<Seen>>> = Arc::default();
    let (log, handler): (_, Handler) = (seen.clone(), Arc::new(handler));
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (log, handler) = (log.clone(), handler.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                let head_end = loop {
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).into_owned();
                let want = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                while buf.len() < head_end + want {
                    match sock.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    }
                }
                let seen = Seen { head };
                log.lock().unwrap().push(seen.clone());
                match handler(&seen) {
                    Reply::Hang => std::future::pending::<()>().await,
                    Reply::Stall { announce, body } => {
                        let head = format!("HTTP/1.1 200 X\r\nContent-Length: {announce}\r\n\r\n");
                        let _ = sock.write_all(head.as_bytes()).await;
                        let _ = sock.write_all(&body).await;
                        std::future::pending::<()>().await
                    }
                    Reply::Is {
                        status,
                        headers,
                        body,
                        announce,
                    } => {
                        let mut out = format!("HTTP/1.1 {status} X\r\nConnection: close\r\n");
                        if !headers
                            .iter()
                            .any(|(k, _)| k.eq_ignore_ascii_case("transfer-encoding"))
                        {
                            out.push_str(&format!(
                                "Content-Length: {}\r\n",
                                announce.unwrap_or(body.len())
                            ));
                        }
                        for (k, v) in headers {
                            out.push_str(&format!("{k}: {v}\r\n"));
                        }
                        out.push_str("\r\n");
                        let _ = sock.write_all(out.as_bytes()).await;
                        let _ = sock.write_all(&body).await;
                        let _ = sock.shutdown().await;
                    }
                }
            });
        }
    });
    (addr, seen)
}
