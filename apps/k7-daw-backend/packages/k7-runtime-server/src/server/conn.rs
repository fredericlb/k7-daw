//! conn.rs — one socket's life: the handshake (its Origin read), a writer task that drains the outbox with a
//! send timeout, and a reader that pings, screens the Origin, answers each frame and cleans up on close.
//! What a reader gets wrong: tungstenite answers a peer's ping and close only when something is written, so the
//! writer flushes the sink when it ends; a peer that answers no ping for a whole interval is closed.

use super::ctx::{Client, Ctx};
use super::dispatch;
use super::handlers::HandlerTable;
use super::outbox::Frame;
use crate::log::say;
use futures_util::{SinkExt, StreamExt};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::watch;
use tokio::time::{Instant, interval_at, timeout};
use tokio_tungstenite::accept_hdr_async_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::tungstenite::protocol::frame::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;

/// The handshake must finish within this.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);
/// A message larger than this closes the socket: a frame is buffered whole before it is read, a project is megabytes.
const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

/// The timers of a socket: a peer silent for a whole ping interval is closed, and one send may wait this long.
#[derive(Clone, Copy)]
pub struct Timers {
    pub ping_interval: Duration,
    pub send_timeout: Duration,
}

impl Default for Timers {
    fn default() -> Self {
        Timers {
            ping_interval: Duration::from_secs(10),
            send_timeout: Duration::from_secs(5),
        }
    }
}

/// Accepts `stream` as a WebSocket and serves it until it closes.
// The handshake callback's error type is the library's own, and large.
#[allow(clippy::result_large_err)]
pub async fn serve_socket(
    stream: TcpStream,
    ctx: Arc<Ctx>,
    table: Arc<HandlerTable>,
    timers: Timers,
    mut stop: watch::Receiver<bool>,
) {
    let ip = stream
        .peer_addr()
        .map(|a| a.ip().to_string())
        .unwrap_or_default();
    let Some(client) = ctx.register(ip) else {
        return;
    };
    let origin = Arc::new(Mutex::new(None::<String>));
    let seen = origin.clone();
    let config = WebSocketConfig::default()
        .max_message_size(Some(MAX_MESSAGE_BYTES))
        .max_frame_size(Some(MAX_MESSAGE_BYTES));
    let shake = accept_hdr_async_with_config(
        stream,
        move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, resp| {
            let value = req.headers().get("origin").and_then(|v| v.to_str().ok());
            *seen.lock().unwrap_or_else(|e| e.into_inner()) = value.map(str::to_owned);
            Ok(resp)
        },
        Some(config),
    );
    let Ok(Ok(ws)) = timeout(HANDSHAKE_TIMEOUT, shake).await else {
        ctx.forget(&client);
        return;
    };
    let (mut sink, mut source) = ws.split();

    let writer = tokio::spawn({
        let client = client.clone();
        async move {
            while let Some(frame) = client.outbox.next().await {
                let (message, last) = match frame {
                    Frame::Text(t) => (Message::text(&*t), false),
                    Frame::Binary(b) => (Message::Binary(b.to_vec().into()), false),
                    Frame::Ping => (Message::Ping(Default::default()), false),
                    Frame::Close(code, reason) => (
                        Message::Close(Some(CloseFrame {
                            code: CloseCode::from(code),
                            reason: reason.into(),
                        })),
                        true,
                    ),
                };
                match timeout(timers.send_timeout, sink.send(message)).await {
                    Ok(Ok(())) if !last => {}
                    _ => break,
                }
            }
            // Flushes what the library queued itself: the answer to a peer's ping or close.
            let _ = timeout(timers.send_timeout, sink.close()).await;
            client.outbox.shutdown();
        }
    });

    if ctx.gate.refuse_origin(
        client.id,
        origin.lock().unwrap_or_else(|e| e.into_inner()).as_deref(),
    ) {
        client.close(1008, "origin not allowed");
    }

    let mut announced = false;
    let mut closed_with = (1006u16, String::new());
    let mut tick = interval_at(Instant::now() + timers.ping_interval, timers.ping_interval);
    let mut awaiting_pong = false;
    loop {
        tokio::select! {
            () = client.outbox.gone() => break,
            Ok(()) = stop.changed() => client.close(1001, "going away"),
            _ = tick.tick() => {
                // A peer that answered nothing since the last ping is gone.
                if awaiting_pong {
                    break;
                }
                awaiting_pong = true;
                client.outbox.ping();
            }
            next = source.next() => {
                let Some(Ok(message)) = next else { break };
                awaiting_pong = false;
                match message {
                    Message::Text(t) if !ctx.gate.is_refused(client.id) => {
                        dispatch::handle(&ctx, &table, &client, t.as_str(), &mut announced);
                    }
                    Message::Binary(b) if !ctx.gate.is_refused(client.id) => match std::str::from_utf8(&b) {
                        Ok(t) => dispatch::handle(&ctx, &table, &client, t, &mut announced),
                        Err(e) => {
                            client.send(&serde_json::json!({
                                "type": "error",
                                "message": format!("bad json: [json.exception.parse_error.101] {e}"),
                            }));
                        }
                    },
                    Message::Close(frame) => {
                        if let Some(f) = frame {
                            closed_with = (u16::from(f.code), f.reason.to_string());
                        } else {
                            closed_with = (1005, String::new());
                        }
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    finish(&ctx, &client, announced, &closed_with);
    // Gives the writer its moment to flush the answer to the peer's close.
    let _ = timeout(Duration::from_secs(1), writer).await;
}

/// The socket is over: what it was bound to goes, and the engine is told what it no longer serves.
fn finish(ctx: &Ctx, client: &Client, announced: bool, closed_with: &(u16, String)) {
    // The engine's side of a leaving listener or driver is posted by the milestone that owns it.
    ctx.forget(client);
    super::transport::release_held(ctx, client);
    if announced {
        let (code, reason) = closed_with;
        let reason = if reason.is_empty() {
            String::new()
        } else {
            format!(", {reason}")
        };
        say!("[client] disconnected (code {code}{reason})");
    }
}
