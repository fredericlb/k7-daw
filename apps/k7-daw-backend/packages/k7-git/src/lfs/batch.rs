//! batch.rs — git-lfs's batch API over k7's HTTPS client: ask which transfers are due, then PUT, GET and verify.
//! What a reader gets wrong: k7's credential goes to the LFS endpoint's host only; an action's `href` on another
//! host (a storage bucket) is called with the action's own headers and nothing of k7's.

use std::collections::BTreeMap;
use std::io::Read;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http_body_util::BodyExt;
use hyper::body::{Body as HttpBody, Bytes, Frame, Incoming, SizeHint};
use hyper::{Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use super::Transfer;
use super::pointer::Pointer;
use super::store::Store;
use crate::transport::http::{Body, client, full};
use crate::transport::idle::Meter;
use crate::{Error, Net, Result};

const MEDIA: &str = "application/vnd.git-lfs+json";

#[derive(Deserialize)]
struct Action {
    href: String,
    #[serde(default)]
    header: BTreeMap<String, String>,
}

#[derive(Deserialize, Default)]
struct Actions {
    upload: Option<Action>,
    download: Option<Action>,
    verify: Option<Action>,
}

#[derive(Deserialize)]
struct Fault {
    code: u16,
    message: String,
}

#[derive(Deserialize)]
struct Item {
    oid: String,
    #[serde(default)]
    actions: Actions,
    error: Option<Fault>,
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    objects: Vec<Item>,
    transfer: Option<String>,
}

#[derive(Serialize)]
struct Ask<'a> {
    operation: &'a str,
    transfers: [&'a str; 1],
    objects: Vec<serde_json::Value>,
    hash_algo: &'a str,
}

/// A file read in chunks as the connection takes them.
struct FileBody {
    file: std::fs::File,
    left: u64,
    meter: Meter,
}

impl HttpBody for FileBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<std::result::Result<Frame<Bytes>, Self::Error>>> {
        if self.left == 0 {
            return Poll::Ready(None);
        }
        let mut buf = vec![0u8; self.left.min(256 * 1024) as usize];
        match self.file.read(&mut buf) {
            Ok(0) => Poll::Ready(Some(Err(std::io::ErrorKind::UnexpectedEof.into()))),
            Ok(n) => {
                buf.truncate(n);
                self.left -= n as u64;
                self.meter.tick();
                Poll::Ready(Some(Ok(Frame::data(Bytes::from(buf)))))
            }
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }

    fn size_hint(&self) -> SizeHint {
        SizeHint::with_exact(self.left)
    }
}

pub(crate) struct Http {
    net: Arc<Net>,
    user: Option<String>,
    /// The SSH remote whose `git-lfs-authenticate` opens the endpoint.
    remote: String,
    /// The endpoint the last batch went to and the `Authorization` that opened it.
    opened_at: std::sync::Mutex<Option<(Url, Option<String>)>>,
}

/// The scheme, host and port: the origin a granted header may go to, and no other.
fn authority(u: &Url) -> String {
    format!(
        "{}://{}:{}",
        u.scheme(),
        u.host_str().unwrap_or_default().to_lowercase(),
        u.port_or_known_default().unwrap_or(0)
    )
}

/// An address a server names for a transfer: https, or http on this machine, and no password in it.
fn vetted(href: &str) -> Result<Url> {
    let at = Url::parse(href).map_err(|e| Error::BadUrl(e.to_string()))?;
    crate::transport::http_check(&at)?;
    Ok(at)
}

impl Http {
    pub fn new(net: Arc<Net>, user: Option<String>, remote: String) -> Http {
        Http {
            net,
            user,
            remote,
            opened_at: Default::default(),
        }
    }

    fn send(
        &self,
        method: Method,
        url: &Url,
        headers: &[(String, String)],
        body: impl FnOnce(&Meter) -> Body,
    ) -> Result<Response<Incoming>> {
        let meter = Meter::new();
        let http = self.net.http.get_or_init(|| client(&self.net.tls)).clone();
        let mut req = Request::builder()
            .method(method)
            .uri(url.as_str())
            .header("user-agent", "k7-git");
        for (k, v) in headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let req = req
            .body(body(&meter))
            .map_err(|e| Error::BadUrl(e.to_string()))?;
        match self
            .net
            .handle
            .block_on(async { meter.within(self.net.idle, http.request(req)).await })
        {
            None => Err(Error::Network(
                "the LFS server did not answer in time".into(),
            )),
            Some(Err(e)) => Err(Error::Network(format!(
                "the LFS server cannot be reached: {e}"
            ))),
            Some(Ok(r)) => Ok(r),
        }
    }

    fn collect(&self, resp: Response<Incoming>) -> Result<Bytes> {
        let mut body = resp.into_body();
        let mut out = Vec::new();
        loop {
            match self
                .net
                .handle
                .block_on(async { tokio::time::timeout(self.net.idle, body.frame()).await })
            {
                Err(_) => return Err(Error::Network("the LFS server stopped answering".into())),
                Ok(None) => return Ok(Bytes::from(out)),
                Ok(Some(Err(e))) => return Err(Error::Network(e.to_string())),
                Ok(Some(Ok(f))) => {
                    if let Ok(d) = f.into_data() {
                        out.extend_from_slice(&d);
                    }
                }
            }
        }
    }

    /// The endpoint and the headers that open it for `op`, as the host's `git-lfs-authenticate` answers.
    fn opened(&self, op: &str) -> Result<(Url, Vec<(String, String)>)> {
        let g = super::ssh::authenticate(&self.net, &self.remote, self.user.as_deref(), op)?;
        let at = vetted(&g.href)?;
        Ok((
            at,
            g.header
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v))
                .collect(),
        ))
    }

    fn batch(&self, op: &str, objs: &[Pointer]) -> Result<Vec<Item>> {
        let (base, mut headers) = self.opened(op)?;
        let mut url = base.clone();
        url.set_path(&format!(
            "{}/objects/batch",
            base.path().trim_end_matches('/')
        ));
        let ask = Ask {
            operation: op,
            transfers: ["basic"],
            objects: objs
                .iter()
                .map(|o| json!({ "oid": o.oid, "size": o.size }))
                .collect(),
            hash_algo: "sha256",
        };
        let text = serde_json::to_vec(&ask).map_err(|e| Error::Io(e.to_string()))?;
        headers.push(("accept".into(), MEDIA.into()));
        headers.push(("content-type".into(), MEDIA.into()));
        let resp = self.send(Method::POST, &url, &headers, |_| {
            full(Bytes::from(text.clone()))
        })?;
        let status = resp.status();
        if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
            return Err(Error::Auth(
                "the LFS server refused the grant the host gave: try again".into(),
            ));
        }
        if !status.is_success() {
            if status == StatusCode::NOT_FOUND || status == StatusCode::NOT_IMPLEMENTED {
                return Err(Error::LfsStatus {
                    status: status.as_u16(),
                    url: url.to_string(),
                });
            }
            return Err(Error::Network(format!("the LFS server answered {status}")));
        }
        let a: Answer = serde_json::from_slice(&self.collect(resp)?)
            .map_err(|_| Error::Network("the LFS server's answer cannot be read".into()))?;
        if a.transfer.as_deref().is_some_and(|t| t != "basic") {
            return Err(Error::Network(
                "the LFS server wants a transfer k7 does not speak".into(),
            ));
        }
        let opened = headers
            .iter()
            .find(|(k, _)| k == "authorization")
            .map(|(_, v)| v.clone());
        *self.opened_at.lock().unwrap_or_else(|p| p.into_inner()) = Some((url.clone(), opened));
        Ok(a.objects)
    }

    /// The headers of an action: its own, and the grant's only when the action is on the endpoint's own host.
    fn action_headers(&self, a: &Action, at: &Url) -> Vec<(String, String)> {
        let mut h: Vec<(String, String)> = a
            .header
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.clone()))
            .collect();
        let opened = self
            .opened_at
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        if let Some((endpoint, Some(auth))) = opened
            && authority(at) == authority(&endpoint)
            && !h.iter().any(|(k, _)| k == "authorization")
        {
            h.push(("authorization".into(), auth));
        }
        h
    }

    fn put(&self, store: &Store, o: &Pointer, a: &Action) -> Result<()> {
        let at = vetted(&a.href)?;
        let file = std::fs::File::open(store.path(&o.oid))?;
        let mut h = self.action_headers(a, &at);
        h.push(("content-type".into(), "application/octet-stream".into()));
        let left = o.size;
        let r = self.send(Method::PUT, &at, &h, |meter| {
            FileBody {
                file,
                left,
                meter: meter.clone(),
            }
            .boxed()
        })?;
        if !r.status().is_success() {
            return Err(Error::Network(format!(
                "the LFS server refused an upload: {}",
                r.status()
            )));
        }
        Ok(())
    }

    fn verify(&self, o: &Pointer, a: &Action) -> Result<()> {
        let at = vetted(&a.href)?;
        let mut h = self.action_headers(a, &at);
        h.push(("accept".into(), MEDIA.into()));
        h.push(("content-type".into(), MEDIA.into()));
        let text = serde_json::to_vec(&json!({ "oid": o.oid, "size": o.size }))
            .map_err(|e| Error::Io(e.to_string()))?;
        let r = self.send(Method::POST, &at, &h, |_| full(Bytes::from(text)))?;
        if !r.status().is_success() {
            return Err(Error::Network(format!(
                "the LFS server did not verify an upload: {}",
                r.status()
            )));
        }
        Ok(())
    }

    fn get(&self, store: &Store, o: &Pointer, a: &Action) -> Result<()> {
        let at = vetted(&a.href)?;
        let h = self.action_headers(a, &at);
        let r = self.send(Method::GET, &at, &h, |_| full(Bytes::new()))?;
        if !r.status().is_success() {
            return Err(Error::Network(format!(
                "the LFS server refused a download: {}",
                r.status()
            )));
        }
        let mut dl = store.begin()?;
        let mut body = r.into_body();
        loop {
            match self
                .net
                .handle
                .block_on(async { tokio::time::timeout(self.net.idle, body.frame()).await })
            {
                Err(_) => {
                    dl.abandon();
                    return Err(Error::Network("the LFS server stopped sending".into()));
                }
                Ok(None) => break,
                Ok(Some(Err(e))) => {
                    dl.abandon();
                    return Err(Error::Network(e.to_string()));
                }
                Ok(Some(Ok(f))) => {
                    if let Ok(d) = f.into_data()
                        && let Err(e) = dl.write(&d)
                    {
                        dl.abandon();
                        return Err(e.into());
                    }
                    if dl.written() > o.size {
                        dl.abandon();
                        return Err(Error::Network(format!(
                            "the LFS server sent more than the {} bytes of the object",
                            o.size
                        )));
                    }
                }
            }
        }
        store.finish(dl, o)
    }
}

/// The object k7 asked for under this oid, with the size k7 asked: a server names nothing else and decides nothing else.
fn asked(objs: &[Pointer], oid: &str) -> Result<Pointer> {
    objs.iter().find(|o| o.oid == oid).cloned().ok_or_else(|| {
        Error::Network("the LFS server answered about an object k7 did not ask for".into())
    })
}

impl Transfer for Http {
    fn upload(&self, store: &Store, objs: &[Pointer]) -> Result<()> {
        let items = self.batch("upload", objs)?;
        for it in &items {
            if let Some(f) = &it.error {
                return Err(Error::Network(format!(
                    "the LFS server cannot take {}: {} ({})",
                    it.oid, f.message, f.code
                )));
            }
        }
        for it in items {
            let want = asked(objs, &it.oid)?;
            if let Some(up) = &it.actions.upload {
                self.put(store, &want, up)?;
                if let Some(v) = &it.actions.verify {
                    self.verify(&want, v)?;
                }
            }
        }
        Ok(())
    }

    fn download(&self, store: &Store, objs: &[Pointer]) -> Result<()> {
        let items = self.batch("download", objs)?;
        for it in items {
            if let Some(f) = &it.error {
                return Err(Error::Network(format!(
                    "the LFS server has no {}: {} ({})",
                    it.oid, f.message, f.code
                )));
            }
            let want = asked(objs, &it.oid)?;
            match &it.actions.download {
                Some(d) => self.get(store, &want, d)?,
                None => {
                    return Err(Error::Network(format!(
                        "the LFS server offers no download of {}",
                        it.oid
                    )));
                }
            }
        }
        // A server that leaves an object out of its answer has not sent it.
        if let Some(o) = objs.iter().find(|o| !store.has(&o.oid)) {
            return Err(Error::Network(format!(
                "the LFS server did not send {}",
                o.oid
            )));
        }
        Ok(())
    }

    fn probe(&self) -> Result<()> {
        // A valid grant is the host speaking LFS; what its endpoint does with an object is no part of the question.
        self.opened("download").map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scheme_is_part_of_where_a_token_may_go() {
        let u = |s: &str| Url::parse(s).unwrap();
        assert_ne!(
            authority(&u("https://h/x")),
            authority(&u("http://h:443/x"))
        );
        assert_eq!(
            authority(&u("https://H:443/a")),
            authority(&u("https://h/b"))
        );
        assert!(vetted("http://example.com/o").is_err());
        assert!(vetted("http://127.0.0.1:9/o").is_ok());
        assert!(vetted("https://u:pw@example.com/o").is_err());
    }
}
