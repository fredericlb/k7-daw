// trackMix.ts — mixer mutations: set a track's scalar fields (volume/pan/mute/solo) by
// rewriting its file in place, so the change round-trips the same way for every surface
// (TUI watcher, web refetch) that reloads it. A surgical edit, not a re-serialization.

import type { K7Scope } from "@studio/k7x/document";
import { laneEventsAt, laneFolder } from "@studio/song/chainItemEdit";
import { stub } from "@studio/song/editStub";
import { wsEdit } from "@studio/song/yaml/workspaceDoc";
import type { Placement } from "@studio/song-schema/yaml/placementSchema";
import type { MaterialKind } from "./clipResize";

// --- gain / pan conversions: a track's `volume` is a LINEAR GAIN, not dB (1.0 == unity) ----

export const MIN_GAIN_DB = -60; // fader floor; below this we store silence

// +6 dB, no higher: Tracktion's fader position saturates at 1.0 there, so a louder value plays at +6 anyway.
export const MAX_GAIN_DB = 6;
export const MAX_GAIN = 10 ** (MAX_GAIN_DB / 20); // ~1.9953

// linear gain -> dB (‑inf at/below silence).
export function gainToDb(linear: number): number {
  return stub("song-edit/trackMix.gainToDb", linear);
}

// dB -> linear gain, clamped in dB (not on the converted gain) so the ceiling is the one number the fader also uses.
export function dbToGain(db: number): number {
  return stub("song-edit/trackMix.dbToGain", db);
}

// A short human label for a stored pan (0..1, 0.5 == centre): "C", "L50", "R30".
export function panToLabel(pan: number): string {
  return stub("song-edit/trackMix.panToLabel", pan);
}

// --- writing a track's file: a placement is addressed by its POSITION IN BEATS, docs/trackMix.md ---

/** Edit lane `id`'s file in place through `edit` on its root; throws when the lane has no file (`what` names the edit in the message). */
export function editTrackStruct(
  dir: string,
  id: string,
  edit: (doc: K7Scope) => void,
  what: string,
): string {
  // `nodes/` (or a chain project's `tracks/`), because a lane IS a node of a `@meta/…-track` module.
  const found = laneEventsAt(dir, id);
  if (!found)
    throw new Error(
      `track "${id}" has no file in ${laneFolder(dir)}/ — cannot ${what}`,
    );
  wsEdit(found.path, (doc) => edit(doc.at(found.base)));
  return found.path;
}

// Move a placement to ANOTHER lane's file, carried over WHOLE and cut first — docs/trackMix.md.
export function movePlacementToTrack(
  dir: string,
  fromId: string,
  toId: string,
  fromBeats: number,
  toTc: string,
  num: number,
  den: number,
): boolean {
  return stub(
    "song-edit/trackMix.movePlacementToTrack",
    dir,
    fromId,
    toId,
    fromBeats,
    toTc,
    num,
    den,
  );
}

/** Move the placement at `fromBeats` to `toTc`, leaving the rest of the file alone. Returns true if one moved. */
export function moveTrackPlacement(
  dir: string,
  id: string,
  fromBeats: number,
  toTc: string,
  num: number,
  den: number,
): boolean {
  return stub(
    "song-edit/trackMix.moveTrackPlacement",
    dir,
    id,
    fromBeats,
    toTc,
    num,
    den,
  );
}

/** Move every placement starting in [fromBeats, toBeats] by `deltaBeats`, in one rewrite — docs/trackMix.md. */
export function moveTrackPlacementsInRange(
  dir: string,
  id: string,
  fromBeats: number,
  toBeats: number,
  deltaBeats: number,
  num: number,
  den: number,
): number {
  return stub(
    "song-edit/trackMix.moveTrackPlacementsInRange",
    dir,
    id,
    fromBeats,
    toBeats,
    deltaBeats,
    num,
    den,
  );
}

/** Switch the placement at `fromBeats` on or off. Written when true, deleted when false. */
export function setTrackPlacementDisabled(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  disabled: boolean,
): boolean {
  return stub(
    "song-edit/trackMix.setTrackPlacementDisabled",
    dir,
    id,
    fromBeats,
    num,
    den,
    disabled,
  );
}

/** Whether the placement at `fromBeats` plays the material's stretch map — polarity inverted, docs/trackMix.md. */
export function setTrackPlacementWarp(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  warp: boolean,
): boolean {
  return stub(
    "song-edit/trackMix.setTrackPlacementWarp",
    dir,
    id,
    fromBeats,
    num,
    den,
    warp,
  );
}

/** How long the placement at `fromBeats` fades, in beats; zero deletes and a fade is a DURATION, docs/trackMix.md. */
export function setTrackPlacementFade(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  which: "fadeIn" | "fadeOut",
  beats: number,
): boolean {
  return stub(
    "song-edit/trackMix.setTrackPlacementFade",
    dir,
    id,
    fromBeats,
    num,
    den,
    which,
    beats,
  );
}

/** Which slice of its material the placement at `fromBeats` plays — its `from`/`to`, in SECONDS; docs/trackMix.md. */
export function setTrackPlacementWindow(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  win: { from: number; to: number },
  dur = 0,
): boolean {
  return stub(
    "song-edit/trackMix.setTrackPlacementWindow",
    dir,
    id,
    fromBeats,
    num,
    den,
    win,
    dur,
  );
}

/** What a resize touched — ONE event, since the window it writes is the placement's own. */
export interface ResizeReport {
  ref: string; // the material the event plays
  material: string; // its project-relative path
  kind: MaterialKind;
  loops: number; // the pass count afterwards
}

/** Re-time the event at `fromBeats` to span `startBeats..endBeats`, or null when there is none — docs/trackMix.md. */
export function resizeTrackPlacement(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
  startBeats: number,
  endBeats: number,
  tempo: number,
): ResizeReport | null {
  return stub(
    "song-edit/trackMix.resizeTrackPlacement",
    dir,
    id,
    fromBeats,
    num,
    den,
    startBeats,
    endBeats,
    tempo,
  );
}

/** Put `ref` on lane `id` at `start` — a REFERENCE, landing on the set the lane is PLAYING; docs/trackMix.md. */
export function addTrackPlacement(
  dir: string,
  id: string,
  ref: string,
  start: string,
): string {
  return stub("song-edit/trackMix.addTrackPlacement", dir, id, ref, start);
}

/** Put SEVERAL placements on one lane in ONE rewrite — atomic, and in the caller's order; docs/trackMix.md. */
export function addTrackPlacements(
  dir: string,
  id: string,
  // A WHOLE placement and not just where it goes, already schema-checked by the caller.
  placements: Placement[],
): string {
  return stub("song-edit/trackMix.addTrackPlacements", dir, id, placements);
}

/** Remove the placement at `fromBeats`. The clip/sequence itself is NOT touched (only the reference) — deleting the last placement just leaves it unplaced. */
export function removeTrackPlacement(
  dir: string,
  id: string,
  fromBeats: number,
  num: number,
  den: number,
): boolean {
  return stub(
    "song-edit/trackMix.removeTrackPlacement",
    dir,
    id,
    fromBeats,
    num,
    den,
  );
}
