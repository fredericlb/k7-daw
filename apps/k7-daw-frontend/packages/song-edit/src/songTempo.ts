// songTempo.ts — the song's one tempo, read off project.k7x for an edit that addresses an event by
// position: a `start` written in seconds reads as beats only through it (song-schema docs/timecode.md).

import { stub } from "@studio/song/editStub";

/** `project.k7x`'s tempo, 120 when it states none — the compile's own default. */
export function songTempo(dir: string): number {
  return stub("song-edit/songTempo.songTempo", dir);
}
