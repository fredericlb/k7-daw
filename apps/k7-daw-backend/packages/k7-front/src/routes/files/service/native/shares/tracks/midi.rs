//! midi.rs — a track's notes as a Standard MIDI File: one track, the project's tempo and time signature, 480 ticks to the quarter.
//! What a reader gets wrong: the model's beat is the engine's, the denominator note, while the file's tick grid is per quarter.

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};
use serde_json::Value;

use super::notes::Note;

/// Ticks to the quarter note.
pub const PPQ: u16 = 480;

/// Ticks of `beats` of the engine's clock, whose beat is the `denominator` note.
fn ticks(beats: f64, denominator: u64) -> u64 {
    (beats * f64::from(PPQ) * 4.0 / denominator as f64)
        .round()
        .max(0.0) as u64
}

/// The file's bytes for `notes` named `name`, under `doc`'s tempo and time signature.
pub fn midi_bytes(name: &str, doc: &Value, notes: &[Note]) -> Result<Vec<u8>, String> {
    let bpm = doc["tempo"].as_f64().filter(|t| *t > 0.0).unwrap_or(120.0);
    let micros = (60_000_000.0 / bpm).round().clamp(1.0, 16_777_215.0) as u32;
    let top = doc["timeSigNumerator"].as_u64().unwrap_or(4).clamp(1, 255) as u8;
    let bottom = doc["timeSigDenominator"].as_u64().unwrap_or(4).max(1);
    let power = if bottom.is_power_of_two() {
        bottom.trailing_zeros() as u8
    } else {
        2
    };
    // Offs sort before ons at one tick, so a repeated key does not hang.
    let mut timed: Vec<(u64, bool, &Note)> = Vec::with_capacity(notes.len() * 2);
    for n in notes {
        let on = ticks(n.start, bottom);
        timed.push((on, true, n));
        timed.push((
            on.max(ticks(n.start + n.length, bottom)).max(on + 1),
            false,
            n,
        ));
    }
    timed.sort_by_key(|(t, on, n)| (*t, *on, n.key));
    let mut track = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TrackName(name.as_bytes())),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(micros))),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(top, power, 24, 8)),
        },
    ];
    let mut last = 0u64;
    for (t, on, n) in timed {
        let message = if on {
            MidiMessage::NoteOn {
                key: u7::new(n.key),
                vel: u7::new(n.vel),
            }
        } else {
            MidiMessage::NoteOff {
                key: u7::new(n.key),
                vel: u7::new(0),
            }
        };
        track.push(TrackEvent {
            delta: u28::new(
                u32::try_from(t - last)
                    .map_err(|e| e.to_string())?
                    .min(0x0FFF_FFFF),
            ),
            kind: TrackEventKind::Midi {
                channel: u4::new(n.channel),
                message,
            },
        });
        last = t;
    }
    track.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    let mut smf = Smf::new(Header::new(
        Format::SingleTrack,
        Timing::Metrical(u15::new(PPQ)),
    ));
    smf.tracks.push(track);
    let mut out = Vec::new();
    smf.write_std(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
