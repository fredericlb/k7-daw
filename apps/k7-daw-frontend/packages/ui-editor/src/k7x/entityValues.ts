// entityValues.ts (k7x) — every string value of a k7x buffer with the field it belongs to, and every key with its path.
// The scan is Rust's (`buffer.value_tokens`); what stays here is the buffer's own coordinates: a span mapped through the edits since the tree (docs/k7xTree.md).

import type { EditorState } from "@codemirror/state";
import { keyTokensOf, valueTokensOf } from "@studio/k7-wasm/buffer";
import { mapSpan, treeOf } from "./tree";

export interface ValueToken {
  from: number;
  to: number;
  /** The key whose value this is, inherited through lists; null at the root. */
  field: string | null;
  /** Every key above it, outermost first, field included. */
  keyPath: string[];
  /** The string itself, as read. */
  value: string;
}

/** A map KEY that is itself an entity reference, what a graph's sections are made of. */
export interface KeyToken {
  from: number;
  to: number;
  /** The keys above it, outermost first; its own text is `key`. */
  keyPath: string[];
  key: string;
}

/** The inner span of a quoted source: one character off each end when the text is quoted. */
const inner = (quoted: boolean, from: number, to: number) =>
  quoted ? { from: from + 1, to: to - 1 } : { from, to };

/** Every non-empty string value in the buffer, with its field and the keys above it. */
export function valueTokens(state: EditorState): ValueToken[] {
  const tree = treeOf(state);
  if (!tree.root) return [];
  const text = state.doc.toString();
  const out: ValueToken[] = [];
  for (const t of valueTokensOf(tree.text)) {
    const span = mapSpan(tree, t.from, t.to, text);
    if (!span) continue;
    out.push({
      ...inner(t.quoted, span.from, span.to),
      field: t.field,
      keyPath: t.keyPath,
      value: t.value,
    });
  }
  return out;
}

/** Every map key in the buffer with the keys above it, so the caller decides per section which ones name something. */
export function keyTokens(state: EditorState): KeyToken[] {
  const tree = treeOf(state);
  if (!tree.root) return [];
  const text = state.doc.toString();
  const out: KeyToken[] = [];
  for (const t of keyTokensOf(tree.text)) {
    const span = mapSpan(tree, t.from, t.to, text);
    if (!span) continue;
    out.push({
      ...inner(t.quoted, span.from, span.to),
      keyPath: t.keyPath,
      key: t.key,
    });
  }
  return out.sort((a, b) => a.from - b.from);
}
