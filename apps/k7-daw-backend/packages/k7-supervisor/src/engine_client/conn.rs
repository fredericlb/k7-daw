//! conn.rs — one WebSocket to the engine and the single reply it may be waited on for. A reader task reads
//! every frame, so the engine's broadcasts never back up, keeps the one that is the awaited type and names
//! no session (another project's news is not ours), and drops the rest; when the socket ends it drops the
//! waiter, which wakes the call at once instead of at its deadline.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{WebSocketStream, client_async};

/// Dialing, handshake included.
pub const CONNECT: Duration = Duration::from_millis(4000);
/// A frame handed to a socket the engine does not read.
const SEND: Duration = Duration::from_millis(4000);

type Waiter = (String, oneshot::Sender<Value>);

pub struct Conn {
    sink: SplitSink<WebSocketStream<TcpStream>, Message>,
    waiting: Arc<Mutex<Option<Waiter>>>,
    reader: JoinHandle<()>,
    /// The client's generation when this was dialed: a `close` since makes it stale.
    pub generation: u64,
}

impl Drop for Conn {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

/// Whether a frame is a reply for a caller waiting on `ty`: a typed object naming no session.
fn is_reply_for(msg: &Value, ty: &str) -> bool {
    let named = msg
        .get("session")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty());
    !named && msg.get("type").and_then(Value::as_str) == Some(ty)
}

impl Conn {
    /// `None` when nothing answers within `CONNECT`: routine, the engine may not be up yet.
    pub async fn dial(port: u16, generation: u64) -> Option<Conn> {
        let url = format!("ws://127.0.0.1:{port}");
        let dialed = async {
            let tcp = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
            client_async(url, tcp).await.ok().map(|(ws, _)| ws)
        };
        let ws = timeout(CONNECT, dialed).await.ok().flatten()?;
        let (sink, mut stream) = ws.split();
        let waiting: Arc<Mutex<Option<Waiter>>> = Arc::default();
        let held = waiting.clone();
        let reader = tokio::spawn(async move {
            while let Some(Ok(frame)) = stream.next().await {
                let Message::Text(t) = frame else { continue };
                let Ok(msg) = serde_json::from_str::<Value>(t.as_str()) else {
                    continue;
                };
                if !msg.is_object() {
                    continue;
                }
                let mut slot = held.lock().unwrap_or_else(|e| e.into_inner());
                if slot.as_ref().is_some_and(|(ty, _)| is_reply_for(&msg, ty))
                    && let Some((_, tx)) = slot.take()
                {
                    let _ = tx.send(msg);
                }
            }
            // The waiter dropped: the call in flight sees a closed socket now.
            held.lock().unwrap_or_else(|e| e.into_inner()).take();
        });
        Some(Conn {
            sink,
            waiting,
            reader,
            generation,
        })
    }

    pub fn open(&self) -> bool {
        !self.reader.is_finished()
    }

    /// Sends `frame`; false when the socket would not take it.
    pub async fn send(&mut self, frame: &Value) -> bool {
        let text = frame.to_string();
        matches!(
            timeout(SEND, self.sink.send(Message::text(text))).await,
            Ok(Ok(()))
        )
    }

    /// Sends `frame` and returns the receiver of the next reply of type `ty`; None when it did not go.
    pub async fn ask(&mut self, frame: &Value, ty: &str) -> Option<oneshot::Receiver<Value>> {
        let (tx, rx) = oneshot::channel();
        *self.waiting.lock().unwrap_or_else(|e| e.into_inner()) = Some((ty.to_string(), tx));
        self.send(frame).await.then_some(rx)
    }

    pub fn forget(&self) {
        self.waiting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
}
