//! An in-process HTTP server for the large-file leg: it answers the LFS routes of `lfsd` and records each request's path.
//! What a reader gets wrong: git itself never goes over http here (a remote is ssh only); this stands for the storage
//! host an SSH remote's `git-lfs-authenticate` points at.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;

#[derive(Default)]
pub struct Cfg {
    /// Every request answered: its path and whether it carried an authorization header.
    pub seen: Mutex<Vec<(String, bool)>>,
}

pub struct Httpd {
    pub addr: SocketAddr,
    pub cfg: Arc<Cfg>,
    pub lfs: Arc<super::lfsd::LfsState>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Httpd {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Httpd {
    /// The endpoint an SSH grant names for a repository.
    pub fn lfs_url(&self, repo: &str) -> String {
        format!("http://127.0.0.1:{}/{repo}/info/lfs", self.addr.port())
    }

    pub async fn start() -> Httpd {
        Httpd::start_with(Arc::default()).await
    }

    /// A server that shares `lfs` with any other server given the same state.
    pub async fn start_with(lfs: Arc<super::lfsd::LfsState>) -> Httpd {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        let cfg = Arc::new(Cfg::default());
        let c = cfg.clone();
        let state = lfs.clone();
        let task = tokio::spawn(async move {
            while let Ok((s, _)) = l.accept().await {
                let (cfg, st) = (c.clone(), state.clone());
                tokio::spawn(async move {
                    let svc = service_fn(move |req| handle(req, cfg.clone(), st.clone()));
                    let _ = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(s), svc)
                        .await;
                });
            }
        });
        Httpd {
            addr,
            cfg,
            lfs,
            task,
        }
    }
}

fn plain(status: u16) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

async fn handle(
    req: Request<Incoming>,
    cfg: Arc<Cfg>,
    lfs: Arc<super::lfsd::LfsState>,
) -> Result<Response<Full<Bytes>>, std::convert::Infallible> {
    let path = req.uri().path().to_string();
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let auth = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    cfg.seen
        .lock()
        .unwrap()
        .push((path.clone(), auth.is_some()));
    let method = req.method().to_string();
    let body = req
        .into_body()
        .collect()
        .await
        .map(|b| b.to_bytes())
        .unwrap_or_default();
    Ok(lfs
        .route(&method, &path, &host, auth, body)
        .unwrap_or_else(|| plain(404)))
}
