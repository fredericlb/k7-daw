//! hub_jobs.rs — a jobs channel end to end: the agent answers the request ITSELF, the file it names is on the machine
//! before the handler runs, and a file the handler writes is on the server before the reply naming it arrives. The
//! handlers here stand in for a separation (`where`, `upper`, registered by `bed::agent`).

use std::time::Duration;

use k7_supervisor::tunnel::TunnelAgent;
use serde_json::{Map, Value, json};

use crate::bed::{AgentSpec, Client, Dir, Hub, HubOptions, agent};

/// One request with an id of its own, its replies as they come: the last one, or the machine's refusal.
async fn request_job(
    c: &Client,
    mut req: Value,
    is_last: impl Fn(&Map<String, Value>) -> bool,
    seen: &mut Vec<String>,
) -> Result<Map<String, Value>, String> {
    req["id"] = json!("job-1");
    c.send(&req.to_string());
    let end = std::time::Instant::now() + Duration::from_secs(10);
    let mut at = 0;
    while std::time::Instant::now() < end {
        let heard = c.heard();
        while at < heard.len() {
            let Ok(Value::Object(reply)) = serde_json::from_str::<Value>(&heard[at]) else {
                at += 1;
                continue;
            };
            at += 1;
            if reply.get("id") != Some(&json!("job-1")) {
                continue;
            }
            if reply.get("type") == Some(&json!("error")) {
                return Err(reply["error"].as_str().unwrap_or("refused").to_string());
            }
            seen.push(reply["type"].as_str().unwrap_or("").to_string());
            if is_last(&reply) {
                return Ok(reply);
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err("no answer".into())
}

fn of(t: &'static str) -> impl Fn(&Map<String, Value>) -> bool {
    move |r| r.get("type") == Some(&json!(t))
}

struct Rig {
    hub: Hub,
    remote: Dir,
    cache: Dir,
    _agent: TunnelAgent,
}

async fn rig() -> Rig {
    let (remote, cache) = (Dir::new("jobs-remote"), Dir::new("jobs-cache"));
    let hub = Hub::start(HubOptions::default()).await;
    // No engine at all: a jobs channel never dials one.
    let (a, _) = agent(AgentSpec {
        url: &hub.agent_url(),
        id: "e1",
        token: "t",
        secret: "",
        engine_port: 1,
        cache: cache.0.clone(),
        protocol: None,
    });
    hub.until_has("e1").await;
    Rig {
        hub,
        remote,
        cache,
        _agent: a,
    }
}

impl Rig {
    fn roots(&self) -> Value {
        json!([{ "remote": self.remote.s(), "name": "lib", "writable": true }])
    }

    async fn jobs_channel(&self) -> Client {
        self.hub
            .open_ready("e1", self.roots(), "jobs")
            .await
            .expect("a channel")
    }
}

#[tokio::test]
async fn runs_the_request_on_the_machine_and_brings_its_output_back() {
    let r = rig().await;
    std::fs::write(r.remote.join("song.txt"), "la la").unwrap();
    let ch = r.jobs_channel().await;
    let remote = r.remote.s();
    let mut seen = vec![];
    let reply = request_job(
        &ch,
        json!({ "type": "upper", "source": format!("{remote}/song.txt"), "outDir": format!("{remote}/stems") }),
        of("done"),
        &mut seen,
    )
    .await
    .unwrap();
    assert_eq!(seen, ["progress", "done"]);
    assert_eq!(
        reply["files"],
        json!([format!("{remote}/stems/song.txt.up")])
    );
    assert_eq!(
        std::fs::read_to_string(r.remote.join("stems/song.txt.up")).unwrap(),
        "LA LA"
    );
}

#[tokio::test]
async fn delivers_a_replys_raw_as_the_machine_wrote_it_and_maps_the_rest() {
    let r = rig().await;
    std::fs::write(r.remote.join("w.txt"), "w").unwrap();
    let ch = r.jobs_channel().await;
    let remote = r.remote.s();
    let reply = request_job(
        &ch,
        json!({ "type": "where", "source": format!("{remote}/w.txt") }),
        of("where"),
        &mut vec![],
    )
    .await
    .unwrap();
    assert_eq!(reply["path"], format!("{remote}/w.txt"));
    let local = reply["raw"]["path"].as_str().unwrap();
    assert_eq!(local, r.cache.join("lib/w.txt").to_string_lossy());
    assert_eq!(std::fs::read_to_string(local).unwrap(), "w");
}

#[tokio::test]
async fn says_so_when_the_machine_does_not_do_what_was_asked() {
    let r = rig().await;
    let ch = r.jobs_channel().await;
    let said = request_job(&ch, json!({ "type": "nope" }), |_| true, &mut vec![]).await;
    assert!(said.is_err(), "{said:?}");
}

#[tokio::test]
async fn is_reachable_from_a_bound_loopback_path_as_a_hosted_file_service_reaches_it() {
    let r = rig().await;
    std::fs::write(r.remote.join("b.txt"), "b").unwrap();
    let path = r.hub.bind_local("e1", r.roots(), "jobs", false).await;
    let ws = Client::dial(r.hub.port(), &path, None).await.unwrap();
    let remote = r.remote.s();
    let reply = request_job(
        &ws,
        json!({ "type": "upper", "source": format!("{remote}/b.txt"), "outDir": format!("{remote}/out") }),
        of("done"),
        &mut vec![],
    )
    .await
    .unwrap();
    assert_eq!(reply["files"], json!([format!("{remote}/out/b.txt.up")]));
    ws.close();
}
