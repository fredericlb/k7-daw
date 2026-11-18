// types.ts — the metronome's state: its tempo, and where on the waveform's timeline it started.

/** A running metronome: tempo plus the waveform-timeline position it started at. */
export interface Metronome {
  bpm: number;
  startSec: number;
}
