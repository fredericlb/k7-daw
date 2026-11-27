// laneMix.ts — how each lane READS in the timeline: muted, soloed, or silenced by another's solo, a group counting for its members.
// A reader would get wrong that the rule is Rust's (song/src/reads/shell.rs): this file only hands each lane back the answer for it.

import { laneMix as read } from "@studio/k7-wasm/reads";
import type { WebGroup, WebTrack, WebVideoTrack } from "./types";

/** What a lane reads as: its own flags and its group's together. */
export interface Eff {
  mute: boolean;
  solo: boolean;
}

export interface LaneMixLookup {
  /** The lane's mute and solo with its group's. */
  effOf: (t: WebTrack) => Eff;
  /** Muted, its group muted, or soloed away by another lane. */
  silent: (t: WebTrack) => boolean;
  /** A picture lane hidden itself, or hidden because something else is soloed. */
  pictureOff: (t: WebVideoTrack) => boolean;
}

/** The lookups for one set of lanes; solo is mixer-wide, so the live pattern lanes are passed with the authored ones. */
export function laneMixOf(
  tracks: readonly WebTrack[],
  groups: readonly WebGroup[],
  video: readonly WebVideoTrack[],
): LaneMixLookup {
  const { lanes, pictures } = read(
    tracks.map((t) => ({
      id: t.id,
      group: t.group,
      mute: t.mute,
      solo: t.solo,
    })),
    groups,
    video.map((v) => ({ id: v.id, mute: v.mute, solo: v.solo })),
  );
  const mix = new Map(tracks.map((t, i) => [t, lanes[i]]));
  const off = new Map(video.map((v, i) => [v, pictures[i]]));
  // A lane the caller never passed reads by its own flags.
  const own = (t: WebTrack) => ({ mute: t.mute, solo: t.solo });
  return {
    effOf: (t) => {
      const m = mix.get(t);
      return m ? { mute: m.mute, solo: m.solo } : own(t);
    },
    silent: (t) => mix.get(t)?.silent ?? t.mute,
    pictureOff: (t) => off.get(t) ?? t.mute,
  };
}
