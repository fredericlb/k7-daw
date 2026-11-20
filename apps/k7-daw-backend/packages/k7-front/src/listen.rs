//! listen.rs — the front's accept loop: hyper-util's auto builder with a timer, so a connection that sends no
//! headers, or idles between requests, is closed, a request's body has node's 30 minutes, and the open
//! connections are capped. `axum::serve` sets no timer, so none of those limits holds there (`docs/front.md`).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, HttpBody};
use axum::extract::ConnectInfo;
use hyper::body::{Frame, Incoming, SizeHint};
use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto::Builder;
use tower::ServiceExt;

/// node's `headersTimeout`: a request's headers, and the wait for the next one on a kept connection.
pub const HEADERS_TIMEOUT: Duration = Duration::from_secs(60);
/// node's `requestTimeout` in `server.ts`: a 512 MB upload over a slow link fits.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Open connections at once; past it the front stops accepting until one closes.
pub const MAX_CONNECTIONS: usize = 4096;

/// The limits, chosen apart so a test can shorten them.
#[derive(Clone, Copy)]
pub struct Limits {
    pub headers: Duration,
    pub request: Duration,
    pub connections: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            headers: HEADERS_TIMEOUT,
            request: REQUEST_TIMEOUT,
            connections: MAX_CONNECTIONS,
        }
    }
}

/// Serves `app` on `listener` until `shutdown` resolves, then lets the open requests finish.
pub async fn serve(
    listener: tokio::net::TcpListener,
    app: Router,
    limits: Limits,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let mut builder = Builder::new(TokioExecutor::new());
    builder
        .http1()
        .timer(TokioTimer::new())
        .header_read_timeout(limits.headers);
    builder
        .http2()
        .timer(TokioTimer::new())
        .keep_alive_interval(Some(limits.headers));
    let builder = Arc::new(builder);
    let permits = Arc::new(tokio::sync::Semaphore::new(limits.connections));
    let graceful = hyper_util::server::graceful::GracefulShutdown::new();
    tokio::pin!(shutdown);
    loop {
        let permit = tokio::select! {
            _ = &mut shutdown => break,
            p = permits.clone().acquire_owned() => p.expect("the semaphore is never closed"),
        };
        let (stream, peer) = tokio::select! {
            _ = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok(a) => a,
                // Out of descriptors, or a connection reset before it was taken: wait, as axum::serve does.
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
        };
        let _ = stream.set_nodelay(true);
        let app = app.clone();
        let request = limits.request;
        let service = hyper::service::service_fn(move |mut req: hyper::Request<Incoming>| {
            req.extensions_mut().insert(ConnectInfo(peer));
            let req = req.map(|b| Body::new(Deadline::new(b, request)));
            app.clone().oneshot(req)
        });
        let conn = builder
            .serve_connection_with_upgrades(TokioIo::new(stream), service)
            .into_owned();
        let conn = graceful.watch(conn);
        tokio::spawn(async move {
            let _ = conn.await;
            drop(permit);
        });
    }
    graceful.shutdown().await;
    Ok(())
}

/// A request body that errors once its deadline passes, as node's `requestTimeout` cuts a request.
struct Deadline<B> {
    body: B,
    sleep: Pin<Box<tokio::time::Sleep>>,
}

impl<B> Deadline<B> {
    fn new(body: B, after: Duration) -> Self {
        Deadline {
            body,
            sleep: Box::pin(tokio::time::sleep(after)),
        }
    }
}

impl<B> HttpBody for Deadline<B>
where
    B: HttpBody + Unpin,
    B::Error: Into<axum::BoxError>,
{
    type Data = B::Data;
    type Error = axum::BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if self.sleep.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Some(Err(
                "the request took longer than the server allows".into()
            )));
        }
        Pin::new(&mut self.body).poll_frame(cx).map_err(Into::into)
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.body.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn front(limits: Limits) -> (SocketAddr, tokio::sync::oneshot::Sender<()>) {
        let app = Router::new().route(
            "/up",
            axum::routing::post(|body: Body| async move {
                match axum::body::to_bytes(body, 1 << 20).await {
                    Ok(b) => format!("{}", b.len()),
                    Err(_) => "cut".into(),
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let at = listener.local_addr().unwrap();
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(serve(listener, app, limits, async {
            let _ = stopped.await;
        }));
        (at, stop)
    }

    /// A client that opens a connection and never finishes its headers is closed, not held.
    #[tokio::test]
    async fn a_silent_connection_is_closed() {
        let limits = Limits {
            headers: Duration::from_millis(300),
            ..Limits::default()
        };
        let (at, _stop) = front(limits).await;
        let mut s = tokio::net::TcpStream::connect(at).await.unwrap();
        s.write_all(b"POST /up HTTP/1.1\r\nHost: x\r\n")
            .await
            .unwrap();
        let mut buf = Vec::new();
        let read = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut buf)).await;
        assert!(read.is_ok(), "the connection was still open after 5 s");
    }

    /// A body that trickles past the request's deadline is cut.
    #[tokio::test]
    async fn a_body_past_the_deadline_is_cut() {
        let limits = Limits {
            request: Duration::from_millis(300),
            ..Limits::default()
        };
        let (at, _stop) = front(limits).await;
        let mut s = tokio::net::TcpStream::connect(at).await.unwrap();
        s.write_all(b"POST /up HTTP/1.1\r\nHost: x\r\nContent-Length: 100\r\n\r\nabc")
            .await
            .unwrap();
        let mut buf = Vec::new();
        let _ = tokio::time::timeout(Duration::from_secs(5), s.read_to_end(&mut buf)).await;
        assert!(
            String::from_utf8_lossy(&buf).ends_with("cut"),
            "{}",
            String::from_utf8_lossy(&buf)
        );
    }

    /// Past the cap a connection waits to be accepted until another one closes.
    #[tokio::test]
    async fn connections_past_the_cap_wait() {
        let limits = Limits {
            connections: 1,
            ..Limits::default()
        };
        let (at, _stop) = front(limits).await;
        let first = tokio::net::TcpStream::connect(at).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut second = tokio::net::TcpStream::connect(at).await.unwrap();
        second
            .write_all(
                b"POST /up HTTP/1.1\r\nHost: x\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .await
            .unwrap();
        let mut buf = Vec::new();
        let waited =
            tokio::time::timeout(Duration::from_millis(500), second.read_to_end(&mut buf)).await;
        assert!(
            waited.is_err(),
            "the second connection was served past the cap"
        );
        drop(first);
        let served =
            tokio::time::timeout(Duration::from_secs(5), second.read_to_end(&mut buf)).await;
        assert!(served.is_ok() && String::from_utf8_lossy(&buf).starts_with("HTTP/1.1 200"));
    }
}
