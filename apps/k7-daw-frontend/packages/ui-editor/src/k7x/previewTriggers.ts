// previewTriggers.ts (k7x) — which lines get a ▶ to audition a sample: a clip's own `source`, a samples symbol's `clip`, or a slice symbol's [from, to) window.
// The rules are Rust's (`buffer.preview_triggers`); what stays here is the buffer's own coordinates, a line mapped through the edits since the tree.

import type { EditorState } from "@codemirror/state";
import { previewTriggersOf } from "@studio/k7-wasm/buffer";
import type { PreviewReq } from "@studio/web-protocol";
import { mapSpan, treeOf } from "./tree";

export interface PreviewTrigger {
  /** A document offset on the line the ▶ belongs to. */
  pos: number;
  req: PreviewReq;
}

/** Every ▶ the document earns; the rack's shared source is resolved after the walk, since it may sit below. */
export function previewTriggers(state: EditorState): PreviewTrigger[] {
  const tree = treeOf(state);
  if (!tree.root) return [];
  const text = state.doc.toString();
  const out: PreviewTrigger[] = [];
  for (const t of previewTriggersOf(tree.text)) {
    const pos = mapSpan(tree, t.from, t.to, text)?.from;
    if (pos !== undefined) out.push({ pos, req: t.req });
  }
  return out;
}
