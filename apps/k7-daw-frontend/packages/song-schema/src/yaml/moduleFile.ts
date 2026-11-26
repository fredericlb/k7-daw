// moduleFile.ts — EDITING a module file, as k7x. Reading it is the Rust reader's, asked through `file.validate`.
// The writers edit a K7Scope in place, so a knob drag is a one-line diff and every comment stays (docs/moduleFile.md).

import type { K7Scope } from "@studio/k7x/document";
import { currentAlternative } from "../alternatives";

export type { ModuleIssue } from "@studio/k7-wasm/types";

/** Set a list of numbers in the file, written on ONE line whatever its length (docs/moduleFile.md). */
export function setModuleList(
  doc: K7Scope,
  path: (string | number)[],
  values: number[],
): void {
  if (!path.length) throw new Error("setModuleList: empty path");
  doc.setIn(path, values, { inline: true });
}

/** Bind a curve to one of a module's params, or take it away — keyed by PARAM (docs/moduleFile.md). */
export function setModuleAutomation(
  doc: K7Scope,
  param: string,
  curve: string | null,
  trigger?: string,
): void {
  const key = [...automationsHome(doc), "automations"];
  const list = doc.getIn(key);
  const items = Array.isArray(list) ? list : [];
  const at = items.findIndex(
    (it) => String((it as { param?: unknown } | null)?.param) === param,
  );
  if (curve === null) {
    if (at < 0) return;
    // The empty list goes with the last entry: `automations([])` says something was emptied.
    if (items.length === 1) doc.deleteIn(key);
    else doc.deleteIn([...key, at]);
    return;
  }
  const entry: Record<string, string> = { param, curve };
  // Only when it is not the default: an absent trigger already means the module being automated.
  if (trigger) entry.trigger = trigger;
  if (at >= 0) doc.applyIn([...key, at], entry);
  else doc.addIn(key, entry);
}

/** Where the resolved node reads its bindings: the set that is on when it swaps the module or has its own list, the node otherwise. */
function automationsHome(doc: K7Scope): (string | number)[] {
  const data = doc.toJS();
  const at = currentAlternative(
    (data && typeof data === "object" ? data : {}) as {
      alternatives?: unknown;
      alternative?: unknown;
    },
  );
  if (at === 0) return [];
  const set = doc.getIn(["alternatives", at - 1]);
  if (!set || typeof set !== "object" || Array.isArray(set)) return [];
  return "extends" in set || "automations" in set
    ? ["alternatives", at - 1]
    : [];
}

/** Set one value in the file, leaving every other byte alone; `null` removes the key. */
export function setModuleValue(
  doc: K7Scope,
  path: (string | number)[],
  value: unknown,
): void {
  if (!path.length) throw new Error("setModuleValue: empty path");
  if (value === null) doc.deleteIn(path);
  else doc.setIn(path, value);
}
