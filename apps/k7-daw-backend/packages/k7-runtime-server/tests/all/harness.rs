//! harness.rs — a server over the fake engine on its own thread, and a test client on 127.0.0.1 that reads what
//! it sends. The thread that starts the server is its message thread, as in the engine's own process.

use futures_util::{SinkExt, StreamExt};
use k7_runtime_server::engine::{Api, Engine, fake};
use k7_runtime_server::server::ctx::Ctx;
use k7_runtime_server::server::{ServeConfig, start};
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

pub struct Server {
    pub addr: SocketAddr,
    pub engine: Engine,
    pub ctx: Arc<Ctx>,
    release: Option<mpsc::Sender<()>>,
    thread: Option<JoinHandle<i32>>,
}

impl Server {
    /// Up and running its message loop.
    pub fn start(cfg: ServeConfig) -> Server {
        let mut s = Server::held(cfg);
        s.release();
        s
    }

    /// Up and listening, with the message loop not yet running.
    pub fn held(cfg: ServeConfig) -> Server {
        let (up, rx) = mpsc::channel();
        let (release, held) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let running = start(Api::new(fake::api()).unwrap(), cfg).expect("the server starts");
            up.send((running.addr(), running.engine(), running.ctx()))
                .unwrap();
            let _ = held.recv();
            running.run()
        });
        let (addr, engine, ctx) = rx.recv().unwrap();
        Server {
            addr,
            engine,
            ctx,
            release: Some(release),
            thread: Some(thread),
        }
    }

    pub fn release(&mut self) {
        if let Some(release) = self.release.take() {
            release.send(()).unwrap();
        }
    }

    /// A quit, and the exit code the server returned.
    pub fn stop(&mut self) -> i32 {
        self.release();
        self.engine.quit();
        self.thread.take().map_or(-1, |t| t.join().unwrap())
    }

    pub async fn connect(&self) -> Wire {
        Wire::connect(self.addr, None).await
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop();
        }
    }
}

pub struct Wire(pub WebSocketStream<MaybeTlsStream<TcpStream>>);

const WAIT: Duration = Duration::from_secs(5);

impl Wire {
    pub async fn connect(addr: SocketAddr, origin: Option<&str>) -> Wire {
        let mut request = format!("ws://{addr}").into_client_request().unwrap();
        if let Some(o) = origin {
            request.headers_mut().insert("origin", o.parse().unwrap());
        }
        let (ws, _) = connect_async(request).await.expect("the handshake");
        Wire(ws)
    }

    pub async fn send(&mut self, text: &str) {
        self.0.send(Message::text(text)).await.unwrap();
    }

    /// The next text frame as the server printed it.
    pub async fn text(&mut self) -> String {
        loop {
            match tokio::time::timeout(WAIT, self.0.next()).await {
                Ok(Some(Ok(Message::Text(t)))) => return t.as_str().to_string(),
                Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => {}
                other => panic!("expected a text frame, got {other:?}"),
            }
        }
    }

    /// Sends `text` and returns the one reply.
    pub async fn ask(&mut self, text: &str) -> String {
        self.send(text).await;
        self.text().await
    }

    /// The close frame the server ends with, or `None` when the stream just ends.
    pub async fn closed_with(&mut self) -> Option<CloseFrame> {
        loop {
            match tokio::time::timeout(WAIT, self.0.next()).await {
                Ok(Some(Ok(Message::Close(f)))) => return f,
                Ok(Some(Ok(_))) => {}
                Ok(Some(Err(_)) | None) => return None,
                Err(_) => panic!("the server never closed"),
            }
        }
    }
}

/// Polls `f` until it holds or five seconds pass.
pub async fn eventually(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..500 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("never held: {what}");
}
