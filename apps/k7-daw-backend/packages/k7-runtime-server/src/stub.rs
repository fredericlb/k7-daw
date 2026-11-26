//! stub.rs — a stand-in socket: binds the port, does the WebSocket handshake and answers `whoami`, nothing more.
//! It exists so M4 can prove the link on the real host; M2's socket core replaces this file whole.
//! What a reader gets wrong: it is not the wire (no Origin screen, no sessions, no ping), only enough of it to probe.

use crate::engine::Engine;
use crate::wire::{json, version};
use base64::Engine as _;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The accept loop's thread and the flag that ends it.
pub struct Listener {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}

impl Listener {
    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.thread.join();
    }
}

/// Binds loopback `port`, then accepts on a thread of its own.
pub fn listen(port: u16, engine: Engine) -> io::Result<Listener> {
    let socket = TcpListener::bind(("127.0.0.1", port))?;
    socket.set_nonblocking(true)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            match socket.accept() {
                Ok((stream, _)) => {
                    let (engine, flag) = (engine.clone(), flag.clone());
                    thread::spawn(move || {
                        let _ = talk(stream, &engine, &flag);
                    });
                }
                Err(_) => thread::sleep(Duration::from_millis(20)),
            }
        }
    });
    Ok(Listener { stop, thread })
}

fn handshake(s: &mut TcpStream) -> io::Result<()> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") && head.len() < 8192 {
        s.read_exact(&mut byte)?;
        head.push(byte[0]);
    }
    let head = String::from_utf8_lossy(&head).into_owned();
    let key = head
        .lines()
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.eq_ignore_ascii_case("sec-websocket-key")
                .then(|| value.trim().to_string())
        })
        .ok_or_else(|| io::Error::other("no key"))?;
    let accept =
        base64::engine::general_purpose::STANDARD.encode(Sha1::digest(format!("{key}{GUID}")));
    write!(
        s,
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n"
    )
}

fn talk(mut s: TcpStream, engine: &Engine, stop: &AtomicBool) -> io::Result<()> {
    s.set_nonblocking(false)?;
    s.set_read_timeout(Some(Duration::from_secs(5)))?;
    handshake(&mut s)?;
    let mut first = [0u8; 1];
    while !stop.load(Ordering::SeqCst) {
        s.set_read_timeout(Some(Duration::from_millis(200)))?;
        if let Err(e) = s.read_exact(&mut first) {
            if matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ) {
                continue;
            }
            return Err(e);
        }
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        let (opcode, payload) = (first[0] & 0x0f, read_payload(&mut s)?);
        if opcode == 8 {
            return Ok(());
        }
        if opcode == 1 {
            let asked: Value = serde_json::from_slice(&payload).unwrap_or(Value::Null);
            if matches!(asked["type"].as_str(), Some("whoami" | "hello")) {
                text(&mut s, &json::dump(&whoami(engine)))?;
            }
        }
    }
    Ok(())
}

// The rest of one client frame, unmasked; a frame over 64 KiB is refused.
fn read_payload(s: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut b = [0u8; 1];
    s.read_exact(&mut b)?;
    let mut len = usize::from(b[0] & 0x7f);
    if len == 126 {
        let mut ext = [0u8; 2];
        s.read_exact(&mut ext)?;
        len = usize::from(u16::from_be_bytes(ext));
    } else if len == 127 {
        return Err(io::Error::other("frame too large for the stub"));
    }
    let mut mask = [0u8; 4];
    if b[0] & 0x80 != 0 {
        s.read_exact(&mut mask)?;
    }
    let mut payload = vec![0u8; len];
    s.read_exact(&mut payload)?;
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[i % 4];
    }
    Ok(payload)
}

fn whoami(engine: &Engine) -> Value {
    let timing = engine.audio_timing().unwrap_or_default();
    let env = |k: &str| std::env::var(k).unwrap_or_default();
    json!({
        "type": "whoami",
        "id": env("STUDIO_ENGINE_ID"),
        "token": env("STUDIO_ENGINE_TOKEN"),
        "boot": "",
        "protocol": version::ENGINE_PROTOCOL,
        "build": version::build(),
        "bufferSize": timing.buffer_frames,
        "sampleRate": timing.sample_rate,
        "messageThreadMs": engine.message_thread_age_ms().unwrap_or(0.0),
    })
}

fn text(s: &mut TcpStream, body: &str) -> io::Result<()> {
    let n = body.len();
    let mut frame = vec![0x81];
    if n < 126 {
        frame.push(n as u8);
    } else {
        frame.push(126);
        frame.extend_from_slice(&u16::try_from(n).map_err(io::Error::other)?.to_be_bytes());
    }
    frame.extend_from_slice(body.as_bytes());
    s.write_all(&frame)
}
