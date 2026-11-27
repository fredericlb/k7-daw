// useProjectDoc.ts — the compiled doc's slice of shell state, refetched on reloadKey.
// The doc drives the strudel poll gate, the track order
// the engine meters index by, the mixer state for the track-node controls, and the
// recording overlay's fields. Apart from App.tsx, which only wires it.

import { DEFAULT_METRONOME_LEVEL } from "@studio/engine-protocol/build";
import {
  type DocTrack,
  type RecInfo,
  type SeqRef,
  shellView,
} from "@studio/k7-wasm/reads";
import { useEffect, useMemo, useRef } from "react";
import { useProjectDocQuery } from "../queries/projectDoc";
import { isAnswer } from "../readFailure";
import { sameJson } from "../shellEquality";
import type { WebProject } from "../types";
import { useLatest } from "./useLatest";

export type { DocTrack, RecInfo };

export interface ProjectDocState {
  trackOrder: string[];
  metronomeOn: boolean;
  /** How loud the click is, 0.2 … 1 — the footer button's right-click slider sits on this. */
  metronomeLevel: number;
  recInfo: RecInfo;
  docTracks: DocTrack[];
  /** The compiled document itself, or null before the first lands — for the one thing no projection serves: a lane's NOTES (rackLit.ts). */
  doc: WebProject | null;
  /** The modules the Mixer view lines up, in track order — `mixerStrips.ts` decides which. */
  mixerStrips: string[];
  /** The MIDI sequence each track carries ({seqId, start-in-beats}), by track id. */
  midiSeqRef: React.RefObject<Record<string, { seqId: string; start: number }>>;
}

/** Before the first document: what the footer and the recorder show for a song not read yet. */
const NO_REC_INFO: RecInfo = {
  tempo: 120,
  beatsPerBar: 4,
  beatUnit: 4,
  countIn: 0,
  names: {},
};
const NO_IDS: string[] = [];
const NO_TRACKS: DocTrack[] = [];

/** Everything the shell projects out of one document, built once per document — the projection is Rust's (song/src/reads/shell.rs). */
const project = (p: WebProject) =>
  shellView({
    tempo: p.tempo,
    num: p.timeSigNumerator,
    den: p.timeSigDenominator,
    countIn: p.countIn,
    // The projection reads each clip's kind, id and start only: the notes stay here.
    tracks: p.tracks.map((t) => ({
      ...t,
      clips: t.clips.map((c) => ({ type: c.type, id: c.id, start: c.start })),
    })),
  });

export function useProjectDoc(
  /** Which reload counter `reloadKey` counts — the cache's namespace (queries/projectDoc.ts). */
  scope: string,
  reloadKey: number,
  opts: {
    /** Called on every doc load with the fresh seq map — disarm a stale MIDI arm. */
    onMidiSeqs: (
      seqs: Record<string, { seqId: string; start: number }>,
    ) => void;
  },
): ProjectDocState {
  // The SAME read the timeline and the minimap draw from, so the row and the faders can never be one reload apart.
  const { doc, failure } = useProjectDocQuery(scope, reloadKey);
  const derived = useMemo(() => (doc ? project(doc) : null), [doc]);
  // The order keeps its identity while the lanes are the same ones: the meters index by it.
  const order = useRef<string[]>(NO_IDS);
  if (derived && !sameJson(order.current, derived.trackOrder))
    order.current = derived.trackOrder;
  const midiSeqRef = useRef<Record<string, { seqId: string; start: number }>>(
    {},
  );
  // The callbacks ride a ref so a re-created callback doesn't re-run the effect.
  const optsRef = useLatest(opts);

  useEffect(() => {
    if (!derived) return;
    // Every key the map holds has a value: the generated map type only says a key may be absent.
    const seqs = derived.seqByTrack as Record<string, SeqRef>;
    midiSeqRef.current = seqs;
    optsRef.current.onMidiSeqs(seqs);
  }, [derived]);
  // No doc — a project that does not compile — empties what is derived from it; a blip keeps the strips.
  const broken = !!failure && isAnswer(failure);

  return {
    trackOrder: order.current,
    metronomeOn: doc?.metronome ?? false,
    metronomeLevel: doc?.metronomeLevel ?? DEFAULT_METRONOME_LEVEL,
    recInfo: derived?.recInfo ?? NO_REC_INFO,
    docTracks: broken ? NO_TRACKS : (derived?.docTracks ?? NO_TRACKS),
    doc,
    mixerStrips: broken ? NO_IDS : (derived?.mixerStrips ?? NO_IDS),
    midiSeqRef,
  };
}
