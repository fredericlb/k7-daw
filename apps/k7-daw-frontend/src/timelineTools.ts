// timelineTools.ts — what a PRESS on a clip means under a tool: the tool names and their labels, and the words for what Rust decided.
// The rule (which verb's line, which refusal) is apps/k7-daw-wasm/gesture/src/tools.rs; here a note's id becomes a sentence. See apps/k7-daw-frontend/docs/timeline.md.

import {
  type ClipTarget,
  clipToolLine,
  crossfadePair as crossfadeNeighbour,
  type ShotTarget,
  shotToolLine,
  type Tool,
  type ToolNote,
} from "@studio/k7-wasm/gesture";
import { m } from "./paraglide/messages.js";

export type { ClipTarget, ShotTarget, Tool };

/** The pointer is the tool with no verb: it means the grab, the marquee and the caret, as before. */
export const TOOLS = [
  "pointer",
  "cut",
  "erase",
  "mute",
  "crossfade",
] as const satisfies readonly Tool[];

export const isTool = (s: string): s is Tool =>
  (TOOLS as readonly string[]).includes(s);

/** What each tool does, for the strip's tooltip and its accessible name (docs/timeline.md). */
export const TOOL_LABEL: Record<Tool, () => string> = {
  pointer: () => m["tool.pointer"](),
  cut: () => m["tool.cut"](),
  erase: () => m["tool.erase"](),
  mute: () => m["tool.mute"](),
  crossfade: () => m["tool.crossfade"](),
};

/** What a press produces: a line to run, or a refusal to say out loud; `null` is the POINTER, the gesture underneath running instead. */
export type ToolAct = { line?: string; note: string } | null;

/** A note's words: its id names the message, its fields fill it. */
function say(n: ToolNote): string {
  switch (n.id) {
    case "cutAtHead":
      return m["tool.cutAtHead"]({ name: n.name });
    case "cutAtTail":
      return m["tool.cutAtTail"]({ name: n.name });
    case "cutDone":
      return m["tool.cutDone"]({ name: n.name, at: n.at });
    case "erased":
      return m["tool.erased"]({ name: n.name, track: n.track });
    case "muted":
      return m["tool.muted"]({ name: n.name });
    case "unmuted":
      return m["tool.unmuted"]({ name: n.name });
    case "shotNoMute":
      return m["tool.shotNoMute"]();
  }
}

const worded = (act: { line?: string; note: ToolNote } | null): ToolAct =>
  act ? { ...(act.line ? { line: act.line } : {}), note: say(act.note) } : null;

/** What a press on a clip does under `tool`. */
export const toolLine = (tool: Tool, c: ClipTarget): ToolAct =>
  worded(clipToolLine(tool, c));

/** The same tools on a picture lane, each on its `picture …` verb; null leaves the drag underneath to run. */
export const pictureToolLine = (tool: Tool, s: ShotTarget): ToolAct =>
  worded(shotToolLine(tool, s));

/** The two neighbouring events a crossfade press means: the one after on the clip's right half, the one before on its left. */
export function crossfadePair<
  C extends { id: string; start: number; length: number; type: string },
>(clips: readonly C[], c: C, at: number): { a: C; b: C } | null {
  const pair = crossfadeNeighbour(clips, c, at);
  const other = pair ? clips[pair.other] : undefined;
  if (!pair || !other) return null;
  return pair.after ? { a: c, b: other } : { a: other, b: c };
}
