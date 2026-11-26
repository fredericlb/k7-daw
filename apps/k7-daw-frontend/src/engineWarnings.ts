// engineWarnings.ts — an engine warning as a person reads it, in plain words, the engine's own text
// kept for the log. One that names a track or a module of the open project is marked IN PLACE (a
// warning icon on that header or card, counted next to the title); every other is a toast, said once
// per project for the page's life. The engine sends prose and no code: song-schema's `engine`
// module recognises the known kinds, and an unknown text still shows, generically, with the raw
// words after it. See ../docs/shell.md, "Engine warnings".

import type { EngineWarning } from "@studio/engine-browser";
import { readWarning } from "@studio/k7-wasm/engine";
import { showToast } from "@studio/project-shell/toast";
import { m } from "./paraglide/messages.js";

/** The sentence the toast says, in the page's language. */
export function warningSentence(w: EngineWarning): string {
  const k = readWarning(w.message).kind;
  switch (k.kind) {
    case "duplicateId":
      return k.what === "clips"
        ? m["engine.warn.duplicateClip"]()
        : k.what === "tracks"
          ? m["engine.warn.duplicateTrack"]()
          : m["engine.warn.duplicateModule"]();
    case "timeSignature":
      return m["engine.warn.timeSignature"]({ signature: k.signature });
    case "unreadable":
      return m["engine.warn.unreadable"]({ file: k.file });
    case "clipMissing":
      return m["engine.warn.clipMissing"]({ file: k.file });
    case "recordFolder":
      return m["engine.warn.recordFolder"]();
    case "pluginFailed":
      return m["engine.warn.pluginFailed"]({ name: k.name });
    case "automationParam":
      return m["engine.warn.automationParam"]({ param: k.param });
    case "hung":
      return m["engine.warn.hung"]();
    case "internal":
      return m["engine.warn.internal"]();
    case "unknown":
      return m["engine.warn.unknown"]({ text: rawText(w) });
  }
}

/** The engine's own words, its subject first: what the log keeps. */
export const rawText = (w: EngineWarning) =>
  w.subject ? `${w.subject}: ${w.message}` : w.message;

/** Whether a warning is new to `scope` (a project's engine session); remembers it if so. */
export function warningSeen(): (scope: string, w: EngineWarning) => boolean {
  const seen = new Set<string>();
  return (scope, w) => {
    const key = `${scope}\n${rawText(w)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  };
}

// One memory per page: a reconnect or a re-apply re-raises the same warnings, and a remount must not forget them.
const firstTime = warningSeen();

/** Toast a warning the first time `scope` raises it; true when it was shown. */
export function toastEngineWarning(scope: string, w: EngineWarning): boolean {
  if (!firstTime(scope, w)) return false;
  showToast({ message: warningSentence(w), variant: "warning" });
  return true;
}

/** What a subject names in the open project: a module (and the track holding it), or a track. */
export interface Placement {
  track: string;
  module?: string;
}

/** The part of the project document a subject is resolved against. */
export interface PlaceableDoc {
  tracks: { id: string; plugins: { id: string }[] }[];
}

/** Resolve an engine subject (a PluginDoc id, or a track id) against the project; null when it names nothing there. */
export function placeSubject(
  subject: string,
  doc: PlaceableDoc | null | undefined,
): Placement | null {
  if (!subject || !doc) return null;
  for (const t of doc.tracks) {
    if (t.plugins.some((p) => p.id === subject))
      return { track: t.id, module: subject };
    if (t.id === subject) return { track: t.id };
  }
  return null;
}

/** The in-place warnings of a project: per module, per track, and how many there are. */
export interface PlacedWarnings {
  byModule: Record<string, string>;
  byTrack: Record<string, string[]>;
  count: number;
}

/** Sort `{ subject → sentence }` into the ones that name something in the project; the rest are not placed. */
export function placeWarnings(
  sentences: Record<string, string>,
  doc: PlaceableDoc | null | undefined,
): PlacedWarnings {
  const out: PlacedWarnings = { byModule: {}, byTrack: {}, count: 0 };
  for (const [subject, sentence] of Object.entries(sentences)) {
    const at = placeSubject(subject, doc);
    if (!at) continue;
    out.count++;
    if (at.module) out.byModule[at.module] = sentence;
    (out.byTrack[at.track] ??= []).push(sentence);
  }
  return out;
}
