//! held_notes.rs — the notes a socket's `midi-live` holds down, per session and track, so a socket that drops
//! mid-note has them released: a tablet losing Wi-Fi never sends its note-off.
//! What a reader gets wrong: the channel is ignored, since the engine plays every live message on channel 1.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Mutex, MutexGuard};

/// The tracks one socket may hold notes on; past it a new track is not followed, which costs at most a hanging note.
pub const MAX_TRACKS_PER_SOCKET: usize = 64;

/// What a closed socket leaves to undo on one track: note-offs, then a sustain-up when the pedal was down.
#[derive(Debug, PartialEq, Eq)]
pub struct Release {
    pub session: String,
    pub track: String,
    pub messages: Vec<Vec<u8>>,
}

#[derive(Default)]
struct Held {
    notes: BTreeSet<u8>,
    sustain: bool,
}

type Tracks = BTreeMap<(String, String), Held>;

#[derive(Default)]
pub struct HeldNotes(Mutex<HashMap<u64, Tracks>>);

impl HeldNotes {
    fn sockets(&self) -> MutexGuard<'_, HashMap<u64, Tracks>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// One channel message a socket sent to a track, as the engine will play it.
    pub fn sent(&self, socket: u64, session: &str, track: &str, bytes: &[u8]) {
        let Some(&status) = bytes.first() else { return };
        let kind = status & 0xF0;
        let mut all = self.sockets();
        let tracks = all.entry(socket).or_default();
        let key = (session.to_owned(), track.to_owned());
        if !tracks.contains_key(&key) && tracks.len() >= MAX_TRACKS_PER_SOCKET {
            return;
        }
        let held = tracks.entry(key).or_default();
        if kind == 0x90 && bytes.len() >= 3 && bytes[2] > 0 {
            held.notes.insert(bytes[1] & 0x7F);
        } else if (kind == 0x80 || kind == 0x90) && bytes.len() >= 2 {
            held.notes.remove(&(bytes[1] & 0x7F));
        } else if kind == 0xB0 && bytes.len() >= 3 {
            // CC 64 is the sustain pedal; 120 and 123 end every note, so nothing is left to release.
            if bytes[1] == 64 {
                held.sustain = bytes[2] >= 64;
            }
            if bytes[1] == 120 || bytes[1] == 123 {
                held.notes.clear();
            }
        }
    }

    /// The socket closed: what it left held, to be played as releases, and it is forgotten.
    pub fn closed(&self, socket: u64) -> Vec<Release> {
        let Some(tracks) = self.sockets().remove(&socket) else {
            return Vec::new();
        };
        tracks
            .into_iter()
            .filter(|(_, h)| !h.notes.is_empty() || h.sustain)
            .map(|((session, track), h)| {
                let mut messages: Vec<Vec<u8>> =
                    h.notes.iter().map(|n| vec![0x80, *n, 0]).collect();
                if h.sustain {
                    messages.push(vec![0xB0, 64, 0]);
                }
                Release {
                    session,
                    track,
                    messages,
                }
            })
            .collect()
    }
}
