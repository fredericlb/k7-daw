//! An in-process git LFS server: the batch API, PUT and GET of objects, verify, and the switches the tests turn.
//! What a reader gets wrong: it can sit on two servers at once (one state, two addresses), which is how a test makes
//! an action's `href` name a second host and reads what reaches it; every hit is recorded with the host it came to.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use http_body_util::Full;
use hyper::Response;
use hyper::body::Bytes;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct Hit {
    pub method: String,
    pub path: String,
    pub host: String,
    pub auth: Option<String>,
}

#[derive(Default)]
pub struct LfsState {
    pub objects: Mutex<HashMap<String, Vec<u8>>>,
    pub hits: Mutex<Vec<Hit>>,
    /// The batch answers 404, as a host without LFS does.
    pub no_batch: AtomicBool,
    /// The base address actions name in place of the server's own (a second host).
    pub href_base: Mutex<Option<String>>,
    /// The `Authorization` every request must carry (what `git-lfs-authenticate` granted).
    pub require_auth: Mutex<Option<String>>,
    /// A lie: the server stores what is PUT under any oid and serves it, so a corrupted download can be tested.
    pub corrupt_downloads: AtomicBool,
    /// What a hostile server answers in place of the oid asked for.
    pub evil_oid: Mutex<Option<String>>,
    /// An item left out of the answer, by position.
    pub drop_item: Mutex<Option<usize>>,
    /// Bytes streamed past the object's end.
    pub extra_bytes: std::sync::atomic::AtomicUsize,
    /// Every batch request body, as sent.
    pub asks: Mutex<Vec<Value>>,
    /// The status a batch of size-0 objects only answers (the empty object a probe once asked for); 0 keeps it.
    pub empty_batch_status: AtomicU16,
    /// The status the batch answers in place of its own (0 keeps it).
    pub batch_status: AtomicU16,
    /// The status a PUT of an object answers (0 keeps 200).
    pub put_status: AtomicU16,
    /// The status the verify call answers (0 keeps 200).
    pub verify_status: AtomicU16,
    /// An upload item that carries an error, as a full quota does.
    pub upload_error: AtomicBool,
    /// The transfer the batch answer names (default basic).
    pub transfer: Mutex<Option<String>>,
}

fn answer(status: u16, body: Vec<u8>) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "application/vnd.git-lfs+json")
        .body(Full::new(Bytes::from(body)))
        .unwrap()
}

impl LfsState {
    pub fn has(&self, oid: &str) -> bool {
        self.objects.lock().unwrap().contains_key(oid)
    }

    pub fn put_bytes(&self, bytes: &[u8]) -> String {
        let oid: String = Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        self.objects
            .lock()
            .unwrap()
            .insert(oid.clone(), bytes.to_vec());
        oid
    }

    pub fn hits_on(&self, host_part: &str) -> Vec<Hit> {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|h| h.host.contains(host_part))
            .cloned()
            .collect()
    }

    /// The answer when `path` is an LFS route, none otherwise.
    pub fn route(
        &self,
        method: &str,
        path: &str,
        host: &str,
        auth: Option<String>,
        body: Bytes,
    ) -> Option<Response<Full<Bytes>>> {
        let is_batch = path.ends_with("/info/lfs/objects/batch");
        if !(is_batch || path.starts_with("/lfs-objects/") || path == "/lfs-verify") {
            return None;
        }
        self.hits.lock().unwrap().push(Hit {
            method: method.into(),
            path: path.into(),
            host: host.into(),
            auth: auth.clone(),
        });
        if let Some(want) = self.require_auth.lock().unwrap().clone()
            && auth.as_deref() != Some(&want)
        {
            return Some(answer(401, Vec::new()));
        }
        if is_batch {
            if self.no_batch.load(Ordering::SeqCst) {
                return Some(answer(404, Vec::new()));
            }
            let empties = self.empty_batch_status.load(Ordering::SeqCst);
            let only_empty = serde_json::from_slice::<Value>(&body)
                .ok()
                .is_some_and(|b| {
                    b["objects"]
                        .as_array()
                        .is_some_and(|o| !o.is_empty() && o.iter().all(|x| x["size"] == 0))
                });
            if empties != 0 && only_empty {
                return Some(answer(empties, Vec::new()));
            }
            let forced = self.batch_status.load(Ordering::SeqCst);
            if forced != 0 {
                return Some(answer(forced, Vec::new()));
            }
            return Some(self.batch(host, &body));
        }
        if let Some(oid) = path.strip_prefix("/lfs-objects/") {
            return Some(match method {
                "PUT" if self.put_status.load(Ordering::SeqCst) != 0 => {
                    answer(self.put_status.load(Ordering::SeqCst), Vec::new())
                }
                "PUT" => {
                    self.objects
                        .lock()
                        .unwrap()
                        .insert(oid.to_string(), body.to_vec());
                    answer(200, Vec::new())
                }
                "GET" => match self.objects.lock().unwrap().get(oid) {
                    Some(b) => {
                        let mut b = b.clone();
                        if self.corrupt_downloads.load(Ordering::SeqCst) {
                            b.push(b'!');
                        }
                        b.extend(std::iter::repeat_n(
                            b'x',
                            self.extra_bytes.load(Ordering::SeqCst),
                        ));
                        answer(200, b)
                    }
                    None => answer(404, Vec::new()),
                },
                _ => answer(405, Vec::new()),
            });
        }
        let verify = self.verify_status.load(Ordering::SeqCst);
        Some(answer(if verify == 0 { 200 } else { verify }, Vec::new()))
    }

    fn batch(&self, host: &str, body: &Bytes) -> Response<Full<Bytes>> {
        let ask: Value = serde_json::from_slice(body).unwrap_or(Value::Null);
        self.asks.lock().unwrap().push(ask.clone());
        let op = ask["operation"].as_str().unwrap_or("");
        let base = self
            .href_base
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| format!("http://{host}"));
        let mut out = Vec::new();
        for o in ask["objects"].as_array().cloned().unwrap_or_default() {
            let oid = o["oid"].as_str().unwrap_or("").to_string();
            let size = o["size"].as_u64().unwrap_or(0);
            let href = format!("{base}/lfs-objects/{oid}");
            let mut item = json!({ "oid": oid, "size": size });
            match (op, self.has(&oid)) {
                ("upload", false) if self.upload_error.load(Ordering::SeqCst) => {
                    item["error"] = json!({ "code": 507, "message": "quota exceeded" });
                }
                ("upload", false) => {
                    item["actions"] = json!({
                        "upload": { "href": href, "header": { "X-Test": "put" } },
                        "verify": { "href": format!("{base}/lfs-verify"), "header": { "X-Test": "verify" } },
                    });
                }
                ("download", true) => {
                    item["actions"] =
                        json!({ "download": { "href": href, "header": { "X-Test": "get" } } })
                }
                ("download", false) => {
                    item["error"] = json!({ "code": 404, "message": "no such object" })
                }
                _ => {}
            }
            if let Some(evil) = self.evil_oid.lock().unwrap().clone() {
                item["oid"] = json!(evil);
            }
            out.push(item);
        }
        if let Some(n) = *self.drop_item.lock().unwrap() {
            out.remove(n);
        }
        answer(
            200,
            serde_json::to_vec(&json!({ "transfer": self.transfer.lock().unwrap().clone().unwrap_or_else(|| "basic".into()), "objects": out })).unwrap(),
        )
    }
}
