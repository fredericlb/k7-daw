// reloadScope.ts — the shell's reload signal and the hook that scopes an effect to the part of a command's file list it concerns.
// A reader would get wrong where the rules are: which routes a list moves is Rust's (`reloadMoved`), and `null` files read as re-ask.

import { reloadMoved } from "@studio/k7-wasm/edit";
import type { ChangedFile } from "@studio/web-protocol";
import { useRef } from "react";

/** The shell's reload signal: still a counter, with `files` beside it; null means 'I do not know', which every predicate reads as re-ask. */
export interface Reload {
  key: number;
  files: ChangedFile[] | null;
}

/** The one every unscoped caller wants: something changed, nobody said what. */
export const UNKNOWN_RELOAD: Reload = { key: 0, files: null };

/** Did a file appear or disappear? The only thing `/tree` draws. */
export const movedTheTree = (r: Reload): boolean =>
  reloadMoved("tree", r.files);

/** Could this have changed `/hints`? Yes for anything but a rewrite of a clip, sequence or curve that was already there. */
export const movedTheVocabulary = (r: Reload): boolean =>
  reloadMoved("vocabulary", r.files);

/** Could this have changed the chains: a node, a cable, an input? No when it only rewrote the arrangement. */
export const movedTheChains = (r: Reload): boolean =>
  reloadMoved("chains", r.files);

/** Could this have changed `/surfaces`? Only a file under `surfaces/` is read for it. */
export const movedTheSurfaces = (r: Reload): boolean =>
  reloadMoved("surfaces", r.files);

/** Could this have brought audio to prewarm? A placement, a clip, or a file that is not k7x: an upload, a take, a render. */
export const movedTheAudio = (r: Reload): boolean =>
  reloadMoved("audio", r.files);

/** A counter that moves only when `moved` says this reload concerns it: the dependency to give an effect in place of `reload.key`. */
export function useScopedKey(
  reload: Reload,
  moved: (r: Reload) => boolean,
): number {
  const seen = useRef({ key: -1, n: 0 });
  if (reload.key !== seen.current.key)
    seen.current = {
      key: reload.key,
      n: seen.current.n + (moved(reload) ? 1 : 0),
    };
  return seen.current.n;
}
