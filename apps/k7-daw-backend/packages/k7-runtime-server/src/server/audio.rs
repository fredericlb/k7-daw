//! audio.rs — the audio broadcast's sender: on its own thread it drains each session's master tap and posts the
//! "LSTR" frame (`wire::stream`) as binary to the sockets listening to that session.
//! What a reader gets wrong: only this thread may move a tap's read cursor (the ring has one consumer), which is why
//! an arrival is queued for it rather than applied by the verb; a socket behind is skipped, never waited for.

use super::broadcast::codec_of;
use super::ctx::Ctx;
use crate::log::say;
use crate::wire::stream::{self, Chunk};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// 480 frames = 10 ms at 48 kHz: smaller means more frames for the same audio, larger puts its own duration under the latency.
const MAX_FRAMES: usize = 480;
/// The sender wakes this often and drains everything waiting.
const TICK: Duration = Duration::from_millis(5);
/// How many frames one pass may send before yielding, a bound only against a burst.
const FRAMES_PER_PASS: usize = 32;
/// How much may sit unsent for one listener before frames are skipped, a third of a second of stereo float32.
pub const MAX_QUEUED_BYTES: usize = 512 * 1024;
/// How often the counters of sessions the engine forgot are retired.
const RETIRE_EVERY: Duration = Duration::from_secs(1);

/// The running sender, stopped and joined when the server stops.
pub struct Sender {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Sender {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for Sender {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Starts the sender; `boot` is the instant `captureMs` counts from.
pub fn spawn(ctx: Arc<Ctx>, boot: Instant) -> Sender {
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let thread = std::thread::Builder::new()
        .name("k7-broadcast".into())
        .spawn(move || run(&ctx, boot, &flag))
        .ok();
    Sender { stop, thread }
}

/// What one session's pump keeps between passes.
#[derive(Default)]
struct Counters {
    /// One sequence per session: a shared counter would have each service read the other's frames as dropped.
    seq: HashMap<String, u32>,
    /// Per session, so one session's unprepared tap does not silence the next one's warning.
    warned_no_rate: HashSet<String>,
}

fn run(ctx: &Ctx, boot: Instant, stop: &AtomicBool) {
    let mut counters = Counters::default();
    let mut samples: Vec<f32> = Vec::new();
    let mut last_retire = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        let arrived = std::mem::take(
            &mut *ctx
                .machine
                .arrived
                .lock()
                .unwrap_or_else(|e| e.into_inner()),
        );
        for sid in &arrived {
            ctx.engine.skip_broadcast_to_live(sid);
        }
        // A session's audio goes only to the sockets listening to that session.
        let mut by_session: BTreeMap<String, Vec<(Arc<super::ctx::Client>, u8)>> = BTreeMap::new();
        {
            let b = ctx.bindings();
            for (id, sid) in &b.listeners {
                if let Some(client) = ctx.client(*id) {
                    by_session
                        .entry(sid.clone())
                        .or_default()
                        .push((client, codec_of(&b, *id)));
                }
            }
        }
        for (sid, targets) in &by_session {
            pump(ctx, boot, &mut counters, &mut samples, sid, targets);
        }
        // Retire the counters of sessions the ENGINE has forgotten, never of ones merely unheard.
        if counters.seq.len() > by_session.len() && last_retire.elapsed() >= RETIRE_EVERY {
            last_retire = Instant::now();
            let live: HashSet<String> = ctx.engine.session_ids().into_iter().collect();
            counters.seq.retain(|s, _| live.contains(s));
            counters.warned_no_rate.retain(|s| live.contains(s));
        }
        std::thread::sleep(TICK);
    }
}

fn pump(
    ctx: &Ctx,
    boot: Instant,
    counters: &mut Counters,
    samples: &mut Vec<f32>,
    sid: &str,
    targets: &[(Arc<super::ctx::Client>, u8)],
) {
    // Drain everything waiting, not one frame per pass: a ceiling equal to the production rate has no headroom.
    for _ in 0..FRAMES_PER_PASS {
        let got = ctx.engine.drain_broadcast(sid, MAX_FRAMES, samples);
        if got.frames == 0 {
            break;
        }
        // A rate of 0 means this session's tap has never been prepared, and the reader rejects it.
        if got.sample_rate < 8000.0 {
            if counters.warned_no_rate.insert(sid.to_string()) {
                say!(
                    "[broadcast] the tap on '{sid}' has no sample rate yet — frames withheld until the graph has prepared it"
                );
            }
            break;
        }
        // A hole the tap overwrote becomes a JUMP in the sequence number, so the listener re-cushions; rounded UP.
        let by = 1 + got.lost_frames.div_ceil(MAX_FRAMES) as u32;
        let seq = counters.seq.entry(sid.to_string()).or_default();
        *seq = seq.wrapping_add(by);
        let capture_ms = boot.elapsed().as_secs_f64() * 1000.0;
        let chunk = Chunk {
            samples,
            frames: got.frames,
            sample_rate: got.sample_rate,
        };
        // Each codec encoded once per frame, and only if some listener asked for it.
        let mut encoded: HashMap<u8, Arc<[u8]>> = HashMap::new();
        for (client, codec) in targets {
            let payload = encoded
                .entry(*codec)
                .or_insert_with(|| Arc::from(stream::encode(&chunk, *seq, capture_ms, *codec)));
            client.outbox.post_binary(payload, MAX_QUEUED_BYTES);
        }
        if got.frames < MAX_FRAMES {
            break;
        }
    }
}
