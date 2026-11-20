//! The waveform routes through `routes::answer` over a fake project (2420 to 2432, 2440): the answer and its memo, the
//! refusals, a prewarm to its end, the cancel and the model poll. The fake locator hands out the clips of a folder of WAVs.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::routes::files::service::native::callbacks::BoxFuture;
use axum::body::Body;
use axum::extract::Request;
use http_body_util::BodyExt;
use k7_file_service::media::onsets::Edits;
use serde_json::{Value, json};
use song::clips::ClipDoc;
use song::graph::ModelNeed;

use super::routes::{Hook, Kind, answer};
use super::testkit::{scratch, wav};
use super::waveform::{Located, Locator, ProjectView, Request as Asked, Why};
use super::{Config, Media};

fn clip(id: &str, source: &Path) -> ClipDoc {
    serde_json::from_value(json!({
        "id": id, "type": "audio", "name": id, "source": source.display().to_string(), "start": 0, "length": 4,
        "offset": 0, "fadeIn": 0, "fadeOut": 0, "loopStart": 0, "loopEnd": 0, "notes": []
    }))
    .unwrap()
}

/// A project of the WAVs of a folder: clip `c0`, `c1`, ... each its own file, and an unknown id declines.
struct Fake {
    dir: PathBuf,
    n: usize,
    edits: Arc<std::sync::Mutex<Edits>>,
}

impl Fake {
    fn source(&self, i: usize) -> PathBuf {
        self.dir.join(format!("c{i}.wav"))
    }
}

impl Locator for Fake {
    fn locate(&self, req: Asked) -> BoxFuture<Result<Located, Why>> {
        let found = (0..self.n).find(|i| req.clip == format!("c{i}"));
        let (dir, found) = (self.dir.clone(), found.map(|i| self.source(i)));
        let edits = self.edits.lock().unwrap().clone();
        Box::pin(async move {
            let Some(source) = found else {
                return Err(if req.clip == "boom" {
                    Why::Decline("clip-file")
                } else {
                    Why::Refuse(400, format!("no clip '{}'", req.clip))
                });
            };
            Ok(Located {
                project: dir.display().to_string(),
                clip_key: req.clip.clone(),
                clip: clip(&req.clip, &source),
                window: None,
                source,
                edits,
                grid: (120.0, 4.0, 4.0),
                rack: false,
            })
        })
    }

    fn need(&self, node: String) -> BoxFuture<Result<ModelNeed, Why>> {
        let url = "http://127.0.0.1:9/never".to_string();
        let got = match node.as_str() {
            "ghost" => Ok(ModelNeed::NoNode),
            "have" => Ok(ModelNeed::Have),
            "nourl" => Ok(ModelNeed::Needs {
                file: "n.onnx".into(),
                url: None,
            }),
            "net" => Ok(ModelNeed::Needs {
                file: "x/net.onnx".into(),
                url: Some(url),
            }),
            "idle" => Err(Why::Refuse(
                404,
                "no active project — select a project in the workspace first".into(),
            )),
            _ => Err(Why::Decline("graph")),
        };
        Box::pin(async move { got })
    }

    fn project(&self) -> BoxFuture<Result<ProjectView, Why>> {
        let view = ProjectView {
            project: self.dir.display().to_string(),
            clips: (0..self.n)
                .map(|i| clip(&format!("c{i}"), &self.source(i)))
                .collect(),
            roots: vec![self.dir.display().to_string()],
            grid: (120.0, 4.0, 4.0),
        };
        Box::pin(async move { Ok(view) })
    }
}

struct Rig {
    edits: Arc<std::sync::Mutex<Edits>>,
    media: Arc<Media>,
    dir: PathBuf,
    owner: bool,
}

impl Rig {
    fn new(name: &str, n: usize) -> Rig {
        Rig::in_dir(scratch(name), n, None)
    }

    fn in_dir(dir: PathBuf, n: usize, engine: Option<(PathBuf, &Path)>) -> Rig {
        for i in 0..n {
            wav(&dir.join(format!("c{i}.wav")), 1.0, 3.0 + i as f64);
        }
        let env = [(
            "STUDIO_HOME".to_string(),
            crate::routes::files::service::native::tests::slashed(&dir.join("home")),
        )]
        .into();
        let mut config = Config::of(&env, &dir);
        if let Some((bin, log)) = engine {
            config.engine = Some(bin);
            config.env.insert("LOG".into(), log.display().to_string());
            config.env.insert("HOLD".into(), "1".into());
        }
        Rig {
            edits: Arc::default(),
            media: Arc::new(Media::new(config, Arc::new(|| ()))),
            dir,
            owner: true,
        }
    }

    async fn get(&self, kind: Kind, target: &str) -> Result<(u16, Value), Box<Request>> {
        self.send(kind, target, Value::Null).await
    }

    async fn send(
        &self,
        kind: Kind,
        target: &str,
        body: Value,
    ) -> Result<(u16, Value), Box<Request>> {
        let n = std::fs::read_dir(&self.dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|x| x == "wav")
            })
            .count();
        let fake: Arc<dyn Locator> = Arc::new(Fake {
            dir: self.dir.clone(),
            n,
            edits: self.edits.clone(),
        });
        let (dir, owner) = (self.dir.clone(), self.owner);
        let resolve = move |_: &hyper::HeaderMap| Ok(("ws".to_string(), dir.display().to_string()));
        let gate = move || owner;
        let locator = move |_: &str, _: &str, _: &str| fake.clone();
        let log: k7_hub::Log = Arc::new(|_: &str| ());
        let hook = Hook {
            media: self.media.clone(),
            account: "acct".into(),
            log: &log,
            resolve: &resolve,
            owner: &gate,
            locator: &locator,
        };
        let method = if matches!(kind, Kind::Prewarm | Kind::Cancel | Kind::Download) {
            "POST"
        } else {
            "GET"
        };
        let req = Request::builder()
            .method(method)
            .uri(target)
            .header("content-type", "application/json")
            .body(match body {
                Value::Null => Body::empty(),
                b => Body::from(b.to_string()),
            })
            .unwrap();
        let path = target.split('?').next().unwrap();
        let r = answer(&hook, kind, path, req).await.map_err(Box::new)?;
        let status = r.status().as_u16();
        let bytes = r.into_body().collect().await.unwrap().to_bytes();
        Ok((
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        ))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_clip_is_drawn_once_then_answered_from_the_memo_and_a_new_threshold_draws_again() {
    let rig = Rig::new("routes_draw", 1);
    let (status, a) = rig
        .get(Kind::Waveform, "/waveform?clip=c0&buckets=64")
        .await
        .unwrap();
    assert_eq!(status, 200);
    for key in [
        "rms",
        "peak",
        "seconds",
        "intervalMs",
        "grid",
        "transients",
        "detected",
        "onsetEdits",
    ] {
        assert!(a.get(key).is_some(), "{key} in {a}");
    }
    assert_eq!(a["rms"].as_array().unwrap().len(), 64);
    let (_, again) = rig
        .get(Kind::Waveform, "/waveform?clip=c0&buckets=64")
        .await
        .unwrap();
    assert_eq!(a, again);
    assert_eq!(
        rig.media.counts.waveforms.load(Ordering::Relaxed),
        1,
        "the second ask is the memo's"
    );
    assert_eq!(rig.media.waves.held().0, 1);
    let (status, _) = rig
        .get(Kind::Waveform, "/waveform?clip=c0&buckets=64&sensitivity=3")
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(
        rig.media.counts.decodes.load(Ordering::Relaxed),
        2,
        "a new threshold decodes again"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_clips_corrections_ride_in_the_memos_signature() {
    let rig = Rig::new("routes_edits", 1);
    let (_, before) = rig
        .get(Kind::Waveform, "/waveform?clip=c0&buckets=64")
        .await
        .unwrap();
    assert_eq!(before["onsetEdits"], json!({}));
    *rig.edits.lock().unwrap() = Edits {
        added: Some(vec![0.5]),
        removed: None,
    };
    let (_, after) = rig
        .get(Kind::Waveform, "/waveform?clip=c0&buckets=64")
        .await
        .unwrap();
    assert_eq!(
        after["onsetEdits"],
        json!({ "added": [0.5] }),
        "the memo did not serve the old corrections"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_routes_refuse_as_node_does_and_a_decline_hands_the_request_back() {
    let rig = Rig::new("routes_refuse", 1);
    let (status, body) = rig.get(Kind::Waveform, "/waveform").await.unwrap();
    assert_eq!(
        (status, body["error"].as_str()),
        (400, Some("clip required"))
    );
    let (status, body) = rig.get(Kind::Waveform, "/waveform?clip=zz").await.unwrap();
    assert_eq!(
        (status, body["error"].as_str()),
        (400, Some("no clip 'zz'"))
    );
    assert!(
        rig.get(Kind::Waveform, "/waveform?clip=boom")
            .await
            .is_err(),
        "a decline is a refusal"
    );
    assert_eq!(rig.media.counts.declines.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_prewarm_reaches_done_equal_to_total_and_a_cancel_resets_the_status() {
    super::testkit::warm_module().await;
    let rig = Rig::new("routes_prewarm", 3);
    let (status, first) = rig.get(Kind::Prewarm, "/waveforms/prewarm").await.unwrap();
    assert_eq!(status, 200);
    assert_eq!(first, json!({ "warming": true, "done": 0, "total": 3 }));
    let mut last = Value::Null;
    for _ in 0..400 {
        last = rig.get(Kind::Status, "/waveforms/status").await.unwrap().1;
        if last["warming"] == false {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(last, json!({ "warming": false, "done": 3, "total": 3 }));
    for i in 0..3 {
        assert!(rig.dir.join(format!("c{i}.wav.cache")).exists());
    }
    let (_, cancelled) = rig.get(Kind::Cancel, "/waveforms/cancel").await.unwrap();
    assert_eq!(
        cancelled,
        json!({ "warming": false, "done": 0, "total": 0 })
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_model_job_is_read_behind_the_owner_gate() {
    let mut rig = Rig::new("routes_model", 0);
    let (status, body) = rig.get(Kind::Model, "/account/models/nope").await.unwrap();
    assert_eq!(
        (status, body["error"].as_str()),
        (404, Some("no such download")),
        "a job this side does not hold is the TypeScript's own 404"
    );
    let job = rig
        .media
        .downloads
        .start("x/y.onnx", "http://127.0.0.1:9/never");
    let (status, body) = rig
        .get(Kind::Model, &format!("/account/models/{}", job.id))
        .await
        .unwrap();
    assert_eq!((status, body["file"].as_str()), (200, Some("x/y.onnx")));
    rig.owner = false;
    let (status, _) = rig.get(Kind::Model, "/account/models/nope").await.unwrap();
    assert_eq!(status, 403);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_download_is_started_from_the_nodes_module_and_refused_as_node_refuses() {
    let rig = Rig::new("routes_download", 0);
    let ask = |node: Value| rig.send(Kind::Download, "/models/download", json!({ "node": node }));
    let (status, body) = ask(json!("have")).await.unwrap();
    assert_eq!((status, body), (200, json!({ "job": null })));
    let (status, body) = ask(json!("ghost")).await.unwrap();
    assert_eq!(
        (status, body["error"].as_str()),
        (404, Some("no node 'ghost' in this project"))
    );
    let (status, body) = ask(json!("idle")).await.unwrap();
    assert_eq!(status, 404);
    assert!(
        body["error"]
            .as_str()
            .unwrap()
            .starts_with("no active project")
    );
    let (status, body) = ask(json!("nourl")).await.unwrap();
    assert_eq!(
        (status, body["error"].as_str()),
        (
            400,
            Some(
                "'nourl' needs the model 'n.onnx' and its module gives no model_url to fetch it from"
            )
        )
    );
    let (status, body) = ask(json!("net")).await.unwrap();
    assert_eq!(
        (status, body["job"]["file"].as_str()),
        (200, Some("x/net.onnx"))
    );
    let id = body["job"]["id"].as_str().unwrap().to_string();
    let (status, polled) = rig
        .get(Kind::Model, &format!("/account/models/{id}"))
        .await
        .unwrap();
    assert_eq!((status, polled["file"].as_str()), (200, Some("x/net.onnx")));
    // A body the schema would refuse is a 400 in zod's words; a node the session cannot judge is handed back.
    for (body, said) in [
        (
            json!({}),
            "Invalid input: expected string, received undefined",
        ),
        (
            json!({ "node": "" }),
            "Too small: expected string to have >=1 characters",
        ),
        (
            json!({ "node": 3 }),
            "Invalid input: expected string, received number",
        ),
    ] {
        let (status, got) = rig
            .send(Kind::Download, "/models/download", body)
            .await
            .unwrap();
        assert_eq!((status, got["error"].as_str()), (400, Some(said)));
    }
    assert!(ask(json!("graph-refused")).await.is_err());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_prewarm_and_a_request_under_a_linked_folder_are_one_decode() {
    use super::testkit::{engine, starts};
    let real = scratch("routes_link_real");
    let link = real.with_extension("link");
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let log = real.join("log");
    let bin = engine(&real, 8000, "");
    let rig = Rig::in_dir(link.clone(), 1, Some((bin, &log)));
    let (_, first) = rig.get(Kind::Prewarm, "/waveforms/prewarm").await.unwrap();
    assert_eq!(first["total"], 1);
    let (status, _) = rig
        .get(Kind::Waveform, "/waveform?clip=c0&buckets=64")
        .await
        .unwrap();
    assert_eq!(status, 200);
    assert_eq!(starts(&log), 1, "one name for one file, one engine run");
    let _ = std::fs::remove_file(&link);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_prewarm_s_status_is_its_workspaces_own() {
    let rig = Rig::new("routes_status_ws", 2);
    rig.get(Kind::Prewarm, "/waveforms/prewarm").await.unwrap();
    let mine = rig.media.prewarms.status(&rig.dir.display().to_string());
    assert_eq!(mine.total, 2);
    let other = rig.media.prewarms.status("/another/workspace");
    assert_eq!((other.warming, other.done, other.total), (false, 0, 0));
}
