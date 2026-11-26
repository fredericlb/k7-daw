//! machine.rs — the verbs about the machine rather than one project: the sessions it holds, the audio interface and
//! its inputs, the meters and what the audio callback costs, and which instruments are installed.
//! What a reader gets wrong: `list-sessions`, `list-audio-devices` and `io-levels` join a query already waiting on
//! the message thread, whose answer is a broadcast; `audio-cost` and `instrument-status` never touch that thread.

use super::ctx::{Ctx, Request};
use super::handlers::HandlerTable;
use super::msg::Malformed;
use crate::engine::Mt;
use crate::engine::machine::AudioDevice;
use crate::wire::json;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The most refs `instrument-status` answers for.
const MAX_INSTRUMENT_REFS: usize = 256;

/// What the machine verbs share across sockets.
#[derive(Default)]
pub struct MachineState {
    /// The device list as last sent to every client: a poll that finds it unchanged answers its asker alone.
    pub devices_sent: Mutex<String>,
    /// An asker joined a waiting `list-audio-devices`, whose reply must then reach everyone.
    pub devices_joined: AtomicBool,
    /// Sessions whose first listener just arrived, for the sender to start from live.
    pub arrived: Mutex<HashSet<String>>,
    /// A render is running: one at a time.
    pub rendering: AtomicBool,
}

pub fn add(t: &mut HandlerTable) {
    t.add(&["list-sessions"], list_sessions);
    t.add(&["drop-session"], drop_session);
    t.add(&["audio-cost"], audio_cost);
    t.add(&["list-audio-devices"], list_audio_devices);
    t.add(&["io-levels"], io_levels);
    t.add(&["set-input-stereo"], set_input_stereo);
    t.add(&["set-audio-device"], set_audio_device);
    t.add(&["instrument-status"], instrument_status);
}

fn f(x: f32) -> Value {
    json!(f64::from(x))
}

fn list_sessions(r: &Request) -> Result<(), Malformed> {
    let mut ears: HashMap<String, i64> = HashMap::new();
    for sid in r.ctx.bindings().listeners.values() {
        *ears.entry(sid.clone()).or_default() += 1;
    }
    // One listing waiting is enough: it is broadcast, so it answers this asker too.
    let Some(ticket) = r.ctx.pending.admit("list-sessions") else {
        return Ok(());
    };
    let ctx = r.ctx.clone();
    r.ctx.engine.post(move |mt: Mt<'_>| {
        ticket.started();
        let sessions: Vec<Value> = mt
            .session_infos()
            .into_iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "project": if s.loaded { s.project } else { String::new() },
                    "loaded": s.loaded,
                    "playing": s.playing,
                    "tracks": s.tracks,
                    "listeners": ears.get(&s.id).copied().unwrap_or(0),
                })
            })
            .collect();
        ctx.broadcast(&json!({"type": "sessions", "sessions": sessions}));
    });
    Ok(())
}

/// Forget a session outright: the supervisor's way of clearing one an app left behind.
fn drop_session(r: &Request) -> Result<(), Malformed> {
    let session = r.session.clone();
    r.ctx.engine.post(move |mt: Mt<'_>| {
        eprintln!("[session] dropped '{session}'");
        mt.drop_session(&session);
    });
    Ok(())
}

/// What the audio callback costs and the deadline it has, read from atomics so it answers off the message thread.
fn audio_cost(r: &Request) -> Result<(), Malformed> {
    if r.msg.bool_or("reset", false)? {
        r.ctx.engine.reset_callback_cost();
    }
    let c = r.ctx.engine.callback_cost().unwrap_or_default();
    r.client.send(&json!({
        "type": "audio-cost",
        "meanMs": c.mean_seconds * 1000.0,
        "maxMs": c.max_seconds * 1000.0,
        "minMs": c.min_seconds * 1000.0,
        "runs": c.runs,
        "deadlineMs": c.deadline_seconds * 1000.0,
    }));
    Ok(())
}

/// The `audio-devices` reply, in the order the engine lists them.
fn devices_msg(devices: Vec<AudioDevice>) -> Value {
    let list: Vec<Value> = devices
        .into_iter()
        .map(|d| {
            json!({
                "name": d.name,
                "type": d.ty,
                "inputs": d.inputs,
                "outputs": d.outputs,
                "current": d.current,
                "currentInput": d.current_input,
                "isDefault": d.is_default,
            })
        })
        .collect();
    json!({"type": "audio-devices", "devices": list})
}

fn list_audio_devices(r: &Request) -> Result<(), Malformed> {
    // A second asker joins the job already waiting, so that job's reply must reach it too: a broadcast.
    let Some(ticket) = r.ctx.pending.admit("list-audio-devices") else {
        r.ctx.machine.devices_joined.store(true, Ordering::SeqCst);
        return Ok(());
    };
    let (ctx, asker) = (r.ctx.clone(), r.client.clone());
    r.ctx.engine.post(move |mt: Mt<'_>| {
        ticket.started();
        let joined = ctx.machine.devices_joined.swap(false, Ordering::SeqCst);
        let msg = devices_msg(mt.list_audio_devices());
        let text = json::dump(&msg);
        {
            let mut sent = ctx.machine.devices_sent.lock().unwrap_or_else(|e| e.into_inner());
            if *sent == text && !joined {
                asker.send(&msg);
                return;
            }
            *sent = text;
        }
        ctx.broadcast(&msg);
    });
    Ok(())
}

/// A channel set's numbers, those that are numbers; an entry that is not a list is skipped.
fn channel_sets(msg: &Value) -> Vec<Vec<i32>> {
    let Some(Value::Array(sets)) = msg.get("channels") else {
        return Vec::new();
    };
    sets.iter()
        .filter_map(Value::as_array)
        .map(|s| {
            s.iter()
                .filter_map(|c| c.as_i64().or_else(|| c.as_f64().map(|x| x as i64)))
                .map(|c| c.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
                .collect()
        })
        .collect()
}

/// Meter the interface itself: one peak and one pitch per `channels` entry, plus the master, asked for not ridden.
fn io_levels(r: &Request) -> Result<(), Malformed> {
    r.msg.get("channels")?;
    let sets = channel_sets(&r.msg.0);
    // Keyed by the channel sets too: two panels on one session meter different inputs.
    let mut key = format!("io-levels\n{}\n", r.session);
    for s in &sets {
        for c in s {
            key += &format!("{c},");
        }
        key += ";";
    }
    let Some(ticket) = r.ctx.pending.admit(&key) else {
        return Ok(());
    };
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.engine.post(move |mt: Mt<'_>| {
        ticket.started();
        let levels = mt.channel_levels_db(&sets);
        let devices: Vec<Value> = mt
            .input_devices()
            .into_iter()
            .map(|d| json!({"name": d.name, "channels": d.channels, "enabled": d.enabled}))
            .collect();
        let pitches = mt.channel_pitches(&sets);
        // Stamped because "master" is this session's own output level.
        ctx.broadcast_to(
            &session,
            json!({
                "type": "io-levels",
                "inputs": levels.iter().map(|l| f(l.db)).collect::<Vec<_>>(),
                "found": levels.iter().map(|l| l.found).collect::<Vec<_>>(),
                "pitches": pitches.iter().map(|p| f(p.hz)).collect::<Vec<_>>(),
                "clarity": pitches.iter().map(|p| f(p.clarity)).collect::<Vec<_>>(),
                "devices": devices,
                "master": f(mt.master_level_db(&session)),
            }),
        );
    });
    Ok(())
}

/// Group two interface inputs into one stereo device or split them; the layout rides on `io-levels`.
fn set_input_stereo(r: &Request) -> Result<(), Malformed> {
    let channel = r.msg.i64_or("channel", 0)?;
    let channel = channel.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
    let stereo = r.msg.bool_or("stereo", false)?;
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.engine.post(move |mt: Mt<'_>| {
        if !mt.set_input_stereo(channel, stereo) {
            ctx.broadcast_to(
                &session,
                json!({"type": "error", "message": "no audio interface open to group inputs on"}),
            );
        }
    });
    Ok(())
}

/// Open a specific interface; `deviceType` is the backend, not `type`, which is the message kind.
fn set_audio_device(r: &Request) -> Result<(), Malformed> {
    let name = r.msg.str("name")?;
    let ty = r.msg.str("deviceType")?;
    let (ctx, session) = (r.ctx.clone(), r.session.clone());
    r.ctx.engine.post(move |mt: Mt<'_>| {
        let kind = if ty.is_empty() {
            String::new()
        } else {
            format!(" [{ty}]")
        };
        eprintln!("[device] opening '{name}'{kind}");
        // The new list goes to everyone, one interface per process; the refusal only to whoever asked.
        if mt.set_audio_device(&name, &ty) {
            sent_list(&ctx, devices_msg(mt.list_audio_devices()));
        } else {
            ctx.broadcast_to(
                &session,
                json!({"type": "error", "message": format!("audio device not available: {name}")}),
            );
        }
    });
    Ok(())
}

fn sent_list(ctx: &Arc<Ctx>, msg: Value) {
    *ctx.machine
        .devices_sent
        .lock()
        .unwrap_or_else(|e| e.into_inner()) = json::dump(&msg);
    ctx.broadcast(&msg);
}

/// Whether each `@instruments/` ref a project names is on this machine: file stats, answered to the asker alone.
fn instrument_status(r: &Request) -> Result<(), Malformed> {
    let mut status = Map::new();
    if let Some(Value::Array(refs)) = r.msg.get("refs")? {
        for name in refs.iter().filter_map(Value::as_str) {
            if status.len() < MAX_INSTRUMENT_REFS {
                let installed = r.ctx.engine.instrument_installed(name);
                status.insert(name.to_string(), Value::Bool(installed));
            }
        }
    }
    r.client
        .send(&json!({"type": "instrument-status", "status": status}));
    Ok(())
}
