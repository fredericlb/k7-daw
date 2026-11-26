//! broadcast.rs — who listens to the master audio, and what the engine is told when that changes: the `broadcast`
//! verb, and a socket's departure, which a closed tab never announces.
//! What a reader gets wrong: every post to the engine is made UNDER the bindings lock, so a departing listener's
//! `false` cannot land after an arriving one's `true` and silence a listener that is still there.

use super::ctx::{Bindings, Departed, Request};
use super::handlers::HandlerTable;
use super::msg::Malformed;
use crate::engine::{Engine, Mt};
use crate::log::say;
use crate::wire::stream::{CODEC_PCM_F32, CODEC_PCM_S16};

pub fn add(t: &mut HandlerTable) {
    t.add(&["broadcast"], broadcast);
}

/// Whether some listener of `session` asked for a silent output.
pub fn any_silent(b: &Bindings, session: &str) -> bool {
    b.silent
        .iter()
        .any(|id| b.listeners.get(id).is_some_and(|s| s == session))
}

fn listening_to(b: &Bindings, session: &str) -> usize {
    b.listeners.values().filter(|s| *s == session).count()
}

/// Subscribe or unsubscribe THIS socket to the master stream; the tap goes on for the first listener and off after the last.
fn broadcast(r: &Request) -> Result<(), Malformed> {
    let on = r.msg.bool_or("on", false)?;
    let pcm16 = r.msg.str("codec")? == "pcm16";
    let silent = r.msg.bool_or("silent", false)?;
    let id = r.client.id;
    let mut target = r.session.clone();
    let remaining;
    {
        let mut b = r.ctx.bindings();
        // Turning OFF retires the socket's OWN subscription, whatever session the message names.
        if !on && let Some(own) = b.listeners.get(&id) {
            target = own.clone();
        }
        // FIRST for THIS session, counted: another service already listening does not make this one mid-stream.
        let first = on && listening_to(&b, &target) == 0;
        // A socket moving to another session leaves its old one.
        let left = match b.listeners.get(&id) {
            Some(old) if on && *old != target => Some(old.clone()),
            _ => None,
        };
        if on {
            b.listeners.insert(id, target.clone());
        } else {
            b.listeners.remove(&id);
        }
        if on && pcm16 {
            b.codecs.insert(id, CODEC_PCM_S16);
        } else {
            b.codecs.remove(&id);
        }
        if on && silent {
            b.silent.insert(id);
        } else {
            b.silent.remove(&id);
        }
        if let Some(left) = left {
            if listening_to(&b, &left) == 0 {
                let sid = left.clone();
                r.ctx
                    .engine
                    .post(move |mt: Mt<'_>| mt.enable_broadcast(&sid, false));
            }
            let (sid, quiet) = (left.clone(), any_silent(&b, &left));
            r.ctx
                .engine
                .post(move |mt: Mt<'_>| mt.silence_output(&sid, quiet));
        }
        remaining = listening_to(&b, &target);
        if first {
            r.ctx
                .machine
                .arrived
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(target.clone());
        }
        let (sid, want) = (target.clone(), remaining > 0);
        r.ctx
            .engine
            .post(move |mt: Mt<'_>| mt.enable_broadcast(&sid, want));
        let (sid, quiet) = (target.clone(), any_silent(&b, &target));
        r.ctx
            .engine
            .post(move |mt: Mt<'_>| mt.silence_output(&sid, quiet));
    }
    say!(
        "[broadcast] listener {} '{target}' ({remaining} listening to it)",
        if on { "added to" } else { "removed from" }
    );
    Ok(())
}

/// A socket closed with `bindings` locked: its listening and driving end, and the engine is told what it no longer serves.
pub fn leave(engine: &Engine, b: &mut Bindings, id: u64) -> Departed {
    b.codecs.remove(&id);
    b.silent.remove(&id);
    let listener = b.listeners.remove(&id);
    if let Some(sid) = &listener {
        // A closed recorder tab sends no goodbye either: the interface plays again once no silent ear is left.
        let (s, quiet) = (sid.clone(), any_silent(b, sid));
        engine.post(move |mt: Mt<'_>| mt.silence_output(&s, quiet));
        say!(
            "[broadcast] listener gone from '{sid}' ({} listening)",
            b.listeners.len()
        );
        // The last ear on THAT session, not on the engine.
        if listening_to(b, sid) == 0 {
            let s = sid.clone();
            engine.post(move |mt: Mt<'_>| mt.enable_broadcast(&s, false));
        }
    }
    let driver = b.drivers.remove(&id);
    if let Some(sid) = &driver {
        let s = sid.clone();
        engine.post(move |mt: Mt<'_>| mt.remove_driver(&s));
    }
    Departed { listener, driver }
}

/// The frame's codec byte of a listener.
pub fn codec_of(b: &Bindings, id: u64) -> u8 {
    b.codecs.get(&id).copied().unwrap_or(CODEC_PCM_F32)
}
