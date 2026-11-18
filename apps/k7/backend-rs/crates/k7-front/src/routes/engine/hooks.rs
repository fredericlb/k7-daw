//! hooks.rs — the testbed's stand-ins for what the TypeScript suites do in-process: move the clock, call the
//! registry and the hub directly, stub `verify`. Compiled only with `test-hooks`, mounted
//! only by `k7-engine-testbed`; the routes under test are the front's own, unchanged.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use k7_engine::registry::heartbeat::read_heartbeat;
use k7_engine::registry::{ClaimRequest, ClaimResult, HeartbeatInput, Refused};
use k7_engine::tunnel::frames::{Kind, Root};
use serde_json::{Value, json};

use super::engine_of;
use crate::AppState;

/// The testbed's limits, from its environment: what the suites give `createRegistry` and `TunnelHub`.
pub fn configure(opts: &mut k7_engine::CoreOptions) {
    let n = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u64>().ok());
    if let Some(m) = n("K7_ENGINE_TEST_MAX_ENGINES") {
        opts.registry.max_engines = Some(m as usize);
    }
    if let Some(m) = n("K7_ENGINE_TEST_NEW_IDS_PER_IP") {
        opts.registry.new_ids_per_ip_per_minute = Some(m as u32);
    }
    if let Some(m) = n("K7_ENGINE_TEST_MIN_PROTOCOL") {
        opts.min_protocol = m as u32;
    }
    if let Some(m) = n("K7_ENGINE_TEST_PING_MS") {
        opts.ping_ms = m;
    }
    if std::env::var("K7_ENGINE_TEST_MEMORY").as_deref() == Ok("1") {
        opts.registry.state_dir = None;
    }
    if let Ok(dir) = std::env::var("K7_ENGINE_TEST_STATE_DIR") {
        opts.registry.state_dir = Some(dir.into());
    }
    if let (Some(open), Some(bytes), Some(hour), Some(idle)) = (
        n("K7_ENGINE_TEST_UPLOAD_OPEN"),
        n("K7_ENGINE_TEST_UPLOAD_BYTES"),
        n("K7_ENGINE_TEST_UPLOAD_HOUR"),
        n("K7_ENGINE_TEST_UPLOAD_IDLE_MS"),
    ) {
        opts.uploads = k7_engine::tunnel::uploads::UploadLimits {
            max_open: open as usize,
            max_bytes: bytes,
            bytes_per_hour: hour,
            idle_ms: idle,
        };
    }
}

pub fn mount(router: Router, state: Arc<AppState>) -> Router {
    router.merge(
        Router::new()
            .route("/__engine-test/{op}", post(hook))
            .with_state(state),
    )
}

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

fn opt(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

async fn hook(
    State(state): State<Arc<AppState>>,
    Path(op): Path<String>,
    Json(b): Json<Value>,
) -> Json<Value> {
    let engine = engine_of(&state);
    let core = &engine.core;
    let reg = &core.registry;
    let hub = &core.hub;
    let id = s(&b, "id");
    Json(match op.as_str() {
        "clock" => {
            if let Some(n) = b.get("now").and_then(Value::as_f64) {
                core.clock.set_now(n as i64);
            }
            if let Some(n) = b.get("advance").and_then(Value::as_f64) {
                core.clock.set_now(core.clock.now_ms() + n as i64);
            }
            json!({ "now": core.clock.now_ms() })
        }
        "upsert" => {
            let mut body = b.clone();
            if let Some(o) = body.as_object_mut() {
                o.remove("publicIpHint");
            }
            match read_heartbeat(&body) {
                Err(e) => json!({ "ok": false, "error": e }),
                Ok(beat) => match reg.upsert(HeartbeatInput {
                    beat,
                    public_ip_hint: opt(&b, "publicIpHint"),
                    from_loopback: k7_engine::registry::is_loopback_address(
                        opt(&b, "publicIpHint").as_deref(),
                    ),
                }) {
                    Ok(()) => json!({ "ok": true }),
                    Err(w) => json!({ "ok": false, "why": match w {
                        Refused::Token => "token", Refused::Secret => "secret", Refused::Full => "full", Refused::Rate => "rate" } }),
                },
            }
        }
        "get" => match reg.get(&id) {
            None => Value::Null,
            Some(e) => {
                let mut o = json!({ "id": e.id, "token": e.token, "hostname": e.hostname, "legacy": e.legacy });
                if let Some(p) = e.performance {
                    o["performance"] = p;
                }
                o
            }
        },
        "list" => {
            let l = reg.list(opt(&b, "ip").as_deref(), &s(&b, "viewer"));
            Value::Array(l.into_iter().map(|e| Value::Object(e.json)).collect())
        }
        "claim" => {
            let req = ClaimRequest {
                project: opt(&b, "project"),
                holder: s(&b, "holder"),
                secret: opt(&b, "secret"),
            };
            let mut out = match reg.claim(&id, req) {
                ClaimResult::Ok { secret, took_over } => {
                    json!({ "ok": true, "secret": secret, "tookOver": took_over.map(|(h, p)| json!({ "holder": h, "project": p })) })
                }
                ClaimResult::Lost {
                    lost,
                    held_by,
                    took_for,
                } => {
                    json!({ "ok": false, "lost": lost.as_str(), "heldBy": held_by, "tookFor": took_for })
                }
            };
            // An absent field is absent, as the TypeScript's object leaves it.
            if let Some(o) = out.as_object_mut() {
                o.retain(|k, v| !(v.is_null() && (k == "tookOver" || k == "tookFor")));
            }
            out
        }
        "release" => {
            reg.release(&id, &s(&b, "secret"));
            Value::Null
        }
        "holds" => json!(reg.holds(&id, &s(&b, "holder"))),
        "verifyToken" => json!(reg.verify_token(&id, &s(&b, "token"))),
        "verifyTunnel" => {
            json!(reg.verify_tunnel(&id, &s(&b, "token"), opt(&b, "secret").as_deref()))
        }
        "remove" => json!(reg.remove(&id, &s(&b, "token"), opt(&b, "secret").as_deref())),
        "engineForProject" => json!(reg.engine_for_project(&s(&b, "project"))),
        "onThisMachine" => json!(reg.on_this_machine(&id)),
        "binding" => json!(reg.binding(&id, opt(&b, "secret").as_deref())),
        "ownerGet" => reg
            .owner_of(&id)
            .map_or(Value::Null, |o| Value::Object(o.json())),
        "ownerAssign" => json!(match reg.assign_owner(&id, &s(&b, "owner")) {
            k7_engine::registry::owners::Assigned::Same => "same",
            k7_engine::registry::owners::Assigned::New => "new",
            k7_engine::registry::owners::Assigned::Moved => "moved",
        }),
        "ownerShare" => {
            let emails: Vec<String> = b
                .get("emails")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            json!(reg.share(&id, &emails, b.get("everyone").and_then(Value::as_bool)))
        }
        "ownerRelease" => json!(reg.release_owner(&id)),
        "ownersAll" => Value::Array(
            reg.owners()
                .into_iter()
                .map(|(i, o, _)| json!([i, Value::Object(o.json())]))
                .collect(),
        ),
        "ticket" => json!(
            core.tickets
                .issue(&id, &s(&b, "holder"), core.clock.now_ms())
        ),
        "verifyTicket" => match core.tickets.verify(&s(&b, "ticket"), core.clock.now_ms()) {
            Some(t) => json!({ "id": t.id, "holder": t.holder }),
            None => Value::Null,
        },
        "hubHas" => json!(hub.has(&id)),
        "hubRenderer" => json!(hub.renderer()),
        "hubRendersEnabled" => json!(hub.renders_enabled()),
        "hubMarkRendered" => {
            hub.mark_rendered(&id);
            Value::Null
        }
        "hubBind" => {
            let roots: Vec<Root> =
                serde_json::from_value(b.get("roots").cloned().unwrap_or(json!([])))
                    .unwrap_or_default();
            let kind = if s(&b, "kind") == "jobs" {
                Kind::Jobs
            } else {
                Kind::Engine
            };
            let synced = b.get("synced").and_then(Value::as_bool).unwrap_or(false);
            json!(hub.bind_local(opt(&b, "engineId").as_deref(), roots, kind, synced))
        }
        "hubUnbind" => {
            hub.unbind_local(&s(&b, "path"));
            Value::Null
        }
        "hubSignal" => match hub
            .signal(
                &id,
                &s(&b, "sdp"),
                std::time::Duration::from_millis(
                    b.get("timeoutMs").and_then(Value::as_u64).unwrap_or(10_000),
                ),
            )
            .await
        {
            Ok(sdp) => json!({ "sdp": sdp }),
            Err(e) => json!({ "error": e }),
        },
        "verify" => {
            let stub: Option<k7_engine::tunnel::hub::Verify> = match s(&b, "mode").as_str() {
                "all" => Some(Arc::new(|_, _, _| true)),
                "fixed" => {
                    let (want_id, want_token, want_secret) =
                        (s(&b, "id"), s(&b, "token"), opt(&b, "secret"));
                    Some(Arc::new(move |i, t, sec| {
                        i == want_id
                            && t == want_token
                            && (want_secret.is_none() || sec == want_secret.as_deref())
                    }))
                }
                _ => None,
            };
            *core.verify_stub.lock().unwrap_or_else(|e| e.into_inner()) = stub;
            Value::Null
        }
        other => json!({ "error": format!("no hook {other}") }),
    })
}
