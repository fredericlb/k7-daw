//! local.rs — a loopback client on a bound path, piped to a channel of its engine's tunnel, as the hub's `pipe`
//! does it: what the client sends before the engine answers is held (bounded), a `jobs` channel's text is
//! respelt to the tunnel's spelling, a peer that stops reading is closed, and either end closing ends both,
//! a client's close after the frames it held (apps/k7-daw-backend/docs/engine/tunnel.md).

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio_tungstenite::WebSocketStream;

use super::backpressure::{Admit, HeldFrames, OVERLOAD_CLOSE, admit};
use super::channel::ChannelEvent;
use super::frames::{Kind, Root};
use super::hub_tunnel::{CLOSE_DRAIN, Tunnel};
use super::spelling::{SEP, tunnel_spelling};
use crate::frame::Frame;
use crate::ws::{self, Event};

/// What a bound path reaches, resolved at its upgrade.
pub struct Target {
    pub tunnel: Arc<Tunnel>,
    pub roots: Vec<Root>,
    pub kind: Kind,
    pub synced: bool,
}

/// Serves a client whose upgrade passed `Hub::check_local`, until either end lets go.
pub async fn serve<S>(target: Target, ws: WebSocketStream<S>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (client, mut from_client) = ws::spawn(ws);
    // Opened once the handshake succeeded: a refused one leaves no channel open on the machine.
    if !target.tunnel.peer.is_open() {
        client.close(1011, "the engine's tunnel went down");
        return;
    }
    let jobs = target.kind == Kind::Jobs;
    let roots = target.roots.clone();
    let mut channel = target.tunnel.open(target.roots, target.kind, target.synced);
    let ch = channel.tx.clone();
    let mut held = HeldFrames::default();
    let overrun = || {
        ch.close();
        client.close(OVERLOAD_CLOSE, "peer too slow");
    };
    loop {
        tokio::select! {
            ev = from_client.recv() => match ev {
                Some(Event::Frame(f)) => {
                    let f = if jobs && !f.binary {
                        Frame::text(tunnel_spelling(&String::from_utf8_lossy(&f.data), &roots, SEP))
                    } else {
                        f
                    };
                    if ch.is_open() {
                        match admit(ch.buffered(), &f) {
                            Admit::Send => ch.send(&f),
                            Admit::Drop => {}
                            Admit::Close => overrun(),
                        }
                    } else if !held.push(f) {
                        overrun();
                    }
                }
                Some(Event::Pong) => {}
                Some(Event::Closed(_)) | None => {
                    // A client gone before its engine answered still has its frames delivered first, within the bound.
                    if !ch.is_open() && !held.is_empty() {
                        let opened = tokio::time::timeout(CLOSE_DRAIN, async {
                            while let Some(ev) = channel.rx.recv().await {
                                match ev {
                                    ChannelEvent::Open => return true,
                                    ChannelEvent::Close => return false,
                                    ChannelEvent::Frame(_) => {}
                                }
                            }
                            false
                        })
                        .await;
                        if opened == Ok(true) {
                            for f in held.drain() {
                                if admit(ch.buffered(), &f) == Admit::Send {
                                    ch.send(&f);
                                }
                            }
                        }
                    }
                    ch.close();
                    return;
                }
            },
            ev = channel.rx.recv() => match ev {
                Some(ChannelEvent::Open) => {
                    for f in held.drain() {
                        match admit(ch.buffered(), &f) {
                            Admit::Send => ch.send(&f),
                            Admit::Drop => {}
                            Admit::Close => overrun(),
                        }
                    }
                }
                Some(ChannelEvent::Frame(f)) => {
                    if client.is_open() {
                        match admit(client.buffered(), &f) {
                            Admit::Send => client.send(&f),
                            Admit::Drop => {}
                            Admit::Close => overrun(),
                        }
                    }
                }
                Some(ChannelEvent::Close) | None => {
                    client.shut();
                    // Drained until the client's socket ends, so its close completes.
                    while let Some(ev) = from_client.recv().await {
                        if matches!(ev, Event::Closed(_)) { break; }
                    }
                    return;
                }
            },
        }
    }
}
