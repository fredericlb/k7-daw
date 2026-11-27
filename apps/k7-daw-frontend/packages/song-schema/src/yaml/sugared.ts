// sugared.ts — new data applied to a document that may hold short forms, each kept while its expansion is unchanged.
// A reader would get wrong that a short form is written back: one whose expansion changed goes, written in full (docs/sugar.md).

import type { K7Document } from "@studio/k7-wasm/k7x";
import type { Sugar } from "./sugar";

type Data = Record<string, unknown>;

const isRecord = (v: unknown): v is Data =>
  v !== null && typeof v === "object" && !Array.isArray(v);

/** An own key holding a value: an undefined one counts as missing, and so does a key only the prototype has. */
const has = (o: Data, key: string): boolean =>
  Object.hasOwn(o, key) && o[key] !== undefined;

/** Deep equality of plain data, an undefined value counting as a missing key. */
function same(a: unknown, b: unknown): boolean {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b))
    return a.length === b.length && a.every((v, i) => same(v, b[i]));
  if (isRecord(a) && isRecord(b)) {
    const ka = Object.keys(a).filter((k) => a[k] !== undefined);
    const kb = Object.keys(b).filter((k) => b[k] !== undefined);
    return (
      ka.length === kb.length && ka.every((k) => has(b, k) && same(a[k], b[k]))
    );
  }
  return false;
}

/** Makes the document's expanded data equal `after` with the fewest edits, keeping every short form whose expansion holds. */
export function applySugared(
  doc: K7Document,
  after: unknown,
  sugar?: Sugar | null,
): void {
  if (!isRecord(after)) throw new Error("k7x: a k7x file is a map at its root");
  if (!sugar) {
    doc.applyIn([], after);
    return;
  }
  const raw = doc.toJS();
  const expanded = sugar.expand(raw);
  if (same(expanded, after)) return;
  const target: Data = { ...after };
  for (const part of sugar.parts) {
    if (!part.present(raw)) continue;
    if (same(expanded[part.target], after[part.target])) {
      // Still what the write wants: the short form stays, and its target as written beside it.
      target[part.key] = raw[part.key];
      if (part.target !== part.key) {
        if (Object.hasOwn(raw, part.target))
          target[part.target] = raw[part.target];
        else delete target[part.target];
      }
    } else if (part.target !== part.key) {
      // Its expansion changed: the short form goes and its target is written in full, or a stale form would undo the edit.
      doc.deleteIn([part.key]);
    }
  }
  doc.applyIn([], target);
}
