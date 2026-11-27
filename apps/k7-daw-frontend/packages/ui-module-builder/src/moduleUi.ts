// moduleUi.ts — the bridge between a module file's k7x text and the visual widget editor: read its `ui(…)` block into a model, write an edited model back.
// The panel's reading and writing are Rust's (`@studio/k7-wasm/panel`); what stays here is the file's other fields, read off a parsed document.
// A reader would get wrong that `extends` is only a reference: "edit the base module" resolves it to the base's file elsewhere.

import type { WebModuleUI as ModuleUI } from "@studio/graph-model";
import { panelWidgets } from "@studio/graph-model/panel";
import {
  DEFAULT_MODULE_STATUS,
  IO_MODULES,
  META_NS,
  type MODULE_STATUSES,
} from "@studio/k7-wasm/consts";
import { K7Document, type K7Scope } from "@studio/k7-wasm/k7x";
import {
  readParams as readParamsOp,
  readUi as readUiOp,
  writeUi as writeUiOp,
} from "@studio/k7-wasm/panel";
import { moduleStatusOf } from "@studio/k7-wasm/schema";

/** How a module asks the completions to treat it: the words `status:` may say. */
export type ModuleStatus = (typeof MODULE_STATUSES)[number];

export type {
  WebModuleUI as ModuleUI,
  WebWidget as Widget,
  WidgetType,
} from "@studio/graph-model";
export { WIDGETS_SCRIPT } from "@studio/k7-wasm/consts";

/** Does the panel draw a screen anywhere, on any tab? That is what earns widgets.js a tab. */
export function hasLcd(ui: ModuleUI | null): boolean {
  return panelWidgets(ui).some(([, w]) => w.type === "lcdscreen");
}

/** A fresh, empty UI for a file that has none yet — small enough to see the whole grid. */
export const EMPTY_UI: ModuleUI = { cols: 4, rows: 2, widgets: {} };

/** A module file's k7x document. */
export type ModuleDoc = K7Document;

/** The document, or null when the text doesn't parse at all (mid-keystroke); exported so a caller reading several fields parses once. */
export function parseModuleDoc(fileText: string): ModuleDoc | null {
  return docOf(fileText);
}

function docOf(fileText: string): ModuleDoc | null {
  const doc = new K7Document(fileText);
  return doc.errors.length ? null : doc;
}

/** Runs a writer on a buffer's document; a buffer that does not parse is handed back unchanged. */
function editText(fileText: string, edit: (doc: K7Scope) => void): string {
  const doc = docOf(fileText);
  if (!doc) return fileText;
  edit(doc.at([]));
  return doc.toString();
}

/** The authored `ui(…)` block, or an empty grid when there is none this can read. */
export function readUi(fileText: string): ModuleUI {
  return readUiOp(fileText).ui ?? { ...EMPTY_UI, widgets: {} };
}

/** Whether the `ui(…)` block round-trips; when false the right pane warns rather than misreading it. */
export function hasParsableUi(fileText: string): boolean {
  return readUiOp(fileText).ui !== null;
}

/** True only when a `ui(…)` block is PRESENT and unreadable; absence is not malformed. */
export function hasMalformedUi(fileText: string): boolean {
  return readUiOp(fileText).malformed;
}

/** The `params(…)` block as name -> value as TEXT, never numbers — see docs/builder.md. */
export const readParams = readParamsOp;

/** Rewrite the module's `ui(…)` block from the model, in place where it can be, every other line untouched. */
export const writeUi = (fileText: string, ui: ModuleUI): string =>
  writeUiOp(fileText, ui);

// --- `ui.k7x` panel files ------------------------------------------------------

/** A `ui.k7x`'s panel, or an empty grid when it has none this can read. */
export function readUiFile(text: string): ModuleUI {
  return readUiOp(text, true).ui ?? { ...EMPTY_UI, widgets: {} };
}

/** True only when the file HAS a panel this can't read — an empty file is not malformed. */
export const hasMalformedUiFile = (text: string): boolean =>
  readUiOp(text, true).malformed;

/** Rewrite a `ui.k7x`'s grid statement by statement, so its header comment survives — see docs/builder.md. */
export const writeUiFile = (text: string, ui: ModuleUI): string =>
  writeUiOp(text, ui, true);

/** The `plugin(…)` block a module declares, off the FILE and SHALLOW on purpose — see docs/builder.md. */
export function readPlugin(
  fileText: string,
): { type: string; name: string } | null {
  const raw = docOf(fileText)?.getIn(["plugin"]);
  const p = raw as { type?: unknown; name?: unknown } | null | undefined;
  if (!p || typeof p !== "object") return null;
  if (typeof p.type !== "string" || typeof p.name !== "string") return null;
  return { type: p.type, name: p.name };
}

/** A module's `io(…)` direction, from THREE sources because a module rarely says it — see docs/builder.md. */
export function ioOf(fileText: string, path?: string): string | null {
  return ioOfDoc(docOf(fileText), path);
}

/** `ioOf` over a document already parsed (null when it does not parse), so one parse serves every read. */
export function ioOfDoc(doc: ModuleDoc | null, path?: string): string | null {
  const io = doc?.getIn(["io"]);
  if (typeof io === "string" && io) return io;

  const ref = doc?.getIn(["extends"]);
  const base = typeof ref === "string" && ref ? ref : null;
  const engine = base?.startsWith(`${META_NS}/`)
    ? IO_MODULES[base.slice(META_NS.length + 1)]
    : undefined;
  if (engine) return engine.io;

  // The two-segment shape matched here rather than borrowed: this needs only the pair, not the root.
  const parts = (path ?? "").split(/[/\\]/);
  for (let i = 0; i + 1 < parts.length; i++)
    if (parts[i] === "io" && IO_MODULES[parts[i + 1]])
      return IO_MODULES[parts[i + 1]].io;
  return null;
}

/** The channels an `io("in")` module declares, AS AUTHORED — two spellings; see docs/builder.md. */
export function channelOf(fileText: string): number[] | string | null {
  return channelOfDoc(docOf(fileText));
}

/** `channelOf` over a document already parsed; only the `channel` node is turned into data. */
export function channelOfDoc(doc: ModuleDoc | null): number[] | string | null {
  const v = doc?.getIn(["channel"]);
  if (typeof v === "string") return v || null;
  if (Array.isArray(v)) {
    const ns = v.map(Number).filter((n) => Number.isInteger(n) && n > 0);
    return ns.length ? ns : null;
  }
  return null;
}

/** Point an `io("in")` module at interface channels, written on ONE line — see docs/builder.md. */
export function setChannelIn(fileText: string, channels: number[]): string {
  return editText(fileText, (doc) =>
    doc.setIn(["channel"], channels, { inline: true }),
  );
}

/** How this module asks the completions to treat it, read off the TEXT so an unsaved edit shows. */
export function statusOf(fileText: string): ModuleStatus {
  return moduleStatusOf(docOf(fileText)?.toJS());
}

/** Set it, or CLEAR it — the DEFAULT is never written; see docs/builder.md. */
export function setStatusIn(fileText: string, status: ModuleStatus): string {
  return editText(fileText, (doc) => {
    if (status === DEFAULT_MODULE_STATUS) doc.deleteIn(["status"]);
    else doc.setIn(["status"], status);
  });
}

/** A module file's `extends(…)` ref, for resolving the base's file; null when absent. */
export function extendsRefOf(fileText: string): string | null {
  const ref = docOf(fileText)?.getIn(["extends"]);
  return typeof ref === "string" && ref ? ref : null;
}
