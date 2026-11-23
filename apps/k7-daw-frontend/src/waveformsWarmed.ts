// waveformsWarmed.ts — a count the waveform prewarm bumps each time the backend has cached more, read by the clips.
// What a reader gets wrong: only a clip still drawn blank asks again on a bump; a clip with its picture keeps it.

import { useSyncExternalStore } from "react";

let generation = 0;
const listeners = new Set<() => void>();

/** The prewarm cached more waveforms (or ended): a clip that has none yet may ask again. */
export function waveformsWarmed(): void {
  generation++;
  for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function read(): number {
  return generation;
}

/** The current count, re-rendering its reader on each bump. */
export function useWaveformsWarmed(): number {
  return useSyncExternalStore(subscribe, read, read);
}
