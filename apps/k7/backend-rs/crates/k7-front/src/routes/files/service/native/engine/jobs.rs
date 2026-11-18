//! The `sync` job over the bound machine's jobs path (2212): a loopback socket the hub bound, spoken to by `request_job`,
//! the same code the médiathèque's jobs use over its tunnel link. A channel that will not open is a failed copy (2223).

use std::time::Duration;

use futures_util::stream::SplitStream;
use futures_util::{SinkExt, StreamExt};
use k7_file_service::engine::sync::Reply;
use serde_json::{Map, Value, json};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::routes::engine_reach::job::{JOB_IDLE, JOB_TOTAL, JobChannel, of_type, request_job};

const HANDSHAKE: Duration = Duration::from_secs(10);

/// A socket to the jobs path as a job channel: a writer task owns the sink, so `send_text` never waits.
pub struct WsChannel {
    tx: mpsc::UnboundedSender<Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
}

impl WsChannel {
    pub async fn open(url: &str) -> Result<WsChannel, String> {
        let dialled = tokio::time::timeout(HANDSHAKE, tokio_tungstenite::connect_async(url)).await;
        let (ws, _) = match dialled {
            Ok(Ok(ok)) => ok,
            Ok(Err(e)) => return Err(e.to_string()),
            Err(_) => return Err("the upgrade was never answered".into()),
        };
        let (mut write, read) = ws.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
        tokio::spawn(async move {
            while let Some(m) = rx.recv().await {
                let closing = matches!(m, Message::Close(_));
                if write.send(m).await.is_err() || closing {
                    break;
                }
            }
        });
        Ok(WsChannel { tx, read })
    }
}

impl JobChannel for WsChannel {
    fn send_text(&mut self, s: String) {
        let _ = self.tx.send(Message::text(s));
    }

    async fn recv(&mut self) -> Option<Vec<u8>> {
        loop {
            match self.read.next().await? {
                Ok(Message::Text(t)) => return Some(t.as_str().as_bytes().to_vec()),
                Ok(Message::Binary(b)) => return Some(b.to_vec()),
                Ok(Message::Close(_)) | Err(_) => return None,
                Ok(_) => {}
            }
        }
    }

    fn close(&mut self) {
        let _ = self.tx.send(Message::Close(None));
    }
}

/// Ask the machine to copy `files`; each `syncing` frame goes to `progress`, the `synced` frame ends it.
pub async fn sync(url: &str, files: Vec<String>, mut progress: impl FnMut(&Value) + Send) -> Reply {
    sync_within(url, files, &mut progress, JOB_TOTAL, JOB_IDLE).await
}

/// `sync` with its bounds named, for the tests of a job never answered.
pub async fn sync_within(
    url: &str,
    files: Vec<String>,
    progress: &mut (impl FnMut(&Value) + Send),
    total: Duration,
    idle: Duration,
) -> Reply {
    let mut chan = match WsChannel::open(url).await {
        Ok(c) => c,
        Err(e) => return Reply::Failed(e),
    };
    let body: Map<String, Value> = json!({ "type": "sync", "files": files })
        .as_object()
        .cloned()
        .unwrap_or_default();
    let done = request_job(
        &mut chan,
        body,
        of_type("synced"),
        |r| {
            if r.get("type").and_then(Value::as_str) == Some("syncing") {
                progress(&Value::Object(r.clone()));
            }
        },
        total,
        idle,
    )
    .await;
    chan.close();
    match done {
        Ok(last) => Reply::Synced(Value::Object(last)),
        Err(e) => Reply::Failed(e.0),
    }
}
