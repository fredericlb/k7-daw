//! streams.rs — the body a long answer needs: fed frame by frame from a queue (an event stream), and `closed`, which
//! ends it on the front's stop, since a graceful shutdown waits for every answer still streaming.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::body::{Body, Bytes, Frame};
use tokio::sync::{mpsc, watch};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// Resolves once the front starts stopping, or its stop signal is gone.
pub fn closed(mut closing: watch::Receiver<bool>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let _ = closing.wait_for(|c| *c).await;
    })
}

/// A body whose frames are pushed into a queue; it ends when every sender is gone.
pub struct ChannelBody(pub mpsc::Receiver<Bytes>);

impl Body for ChannelBody {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, BoxError>>> {
        self.0.poll_recv(cx).map(|b| b.map(|b| Ok(Frame::data(b))))
    }
}
