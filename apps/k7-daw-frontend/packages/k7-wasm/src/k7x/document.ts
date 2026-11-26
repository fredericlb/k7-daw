// document.ts — a k7x file as a document whose values are set, added, moved and deleted by key path, every other byte kept.
// A reader would get wrong where an edit runs: each write is a `k7x.edit` of the Rust Document; reads come from `k7x.data` and the outline.
// The contract every writer relies on: a knob drag is a one-line diff and a comment survives every edit (k7x's docs/rustEdit.md).

import { call } from "../abi.ts";
import type { EditAnswer, EditOp, K7Change } from "../types/index.ts";
import {
  type K7Path,
  type K7SyntaxError,
  lineCol,
  readData,
  syntaxError,
  toWire,
} from "./read.ts";
import {
  indexOf,
  type K7MapNode,
  type K7Node,
  nodeAt,
  parseTree,
} from "./tree.ts";

export type { K7Change };

/** How a written value is laid out: `inline` keeps it on one line whatever its width. */
export interface WriteOptions {
  inline?: boolean;
}

/** One batch of edits on a text: the new text and its splices, or the batch's first failure thrown after the ops before it. */
export function editText(text: string, ops: EditOp[]): EditAnswer {
  return call<EditAnswer>({ op: "k7x.edit", args: { text, ops } });
}

const BOM = String.fromCharCode(0xfeff);

/** The child of a plain value at one path segment, as a tree node's child would be found. */
function childOf(v: unknown, seg: string | number): unknown {
  if (Array.isArray(v)) {
    const i = indexOf(seg);
    return i === undefined ? undefined : v[i];
  }
  if (v !== null && typeof v === "object" && typeof seg === "string")
    return Object.hasOwn(v, seg)
      ? (v as Record<string, unknown>)[seg]
      : undefined;
  return undefined;
}

export class K7Document {
  private current: string;
  /** Every splice applied since construction, in order: what an editor dispatches to keep its undo history. */
  readonly changes: K7Change[] = [];
  private data: Record<string, unknown> | null = null;
  private syntax: K7SyntaxError | null = null;
  private tree: K7MapNode | null = null;

  constructor(text: string) {
    this.current = text;
  }

  get text(): string {
    return this.current;
  }

  private read(): Record<string, unknown> {
    if (this.data) return this.data;
    if (this.syntax) throw this.syntax;
    const a = readData(this.current);
    if (a.syntax) {
      this.syntax = syntaxError(a.syntax);
      throw this.syntax;
    }
    this.data = a.data ?? {};
    return this.data;
  }

  /** The syntax error of a text that does not read: none, or one. */
  get errors(): K7SyntaxError[] {
    try {
      this.read();
      return [];
    } catch (e) {
      if (e === this.syntax && this.syntax) return [this.syntax];
      throw e;
    }
  }

  /** The tree with its spans, or null when the text does not read. */
  get contents(): K7MapNode | null {
    if (this.errors.length) return null;
    this.tree ??= parseTree(this.current);
    return this.tree;
  }

  toString(): string {
    return this.current;
  }

  toJS(): Record<string, unknown> {
    return structuredClone(this.read());
  }

  getNode(path: K7Path): K7Node | undefined {
    const root = this.contents;
    if (!root) throw this.errors[0];
    return nodeAt(root, path);
  }

  getIn(path: K7Path): unknown {
    let v: unknown = this.read();
    for (const seg of path) {
      v = childOf(v, seg);
      if (v === undefined) return undefined;
    }
    return structuredClone(v);
  }

  hasIn(path: K7Path): boolean {
    let v: unknown = this.read();
    for (const seg of path) {
      v = childOf(v, seg);
      if (v === undefined) return false;
    }
    return true;
  }

  /** The exact source of the node at a path; an empty call's flag, `key()`, has none and gives "". */
  getSource(path: K7Path): string | undefined {
    const node = this.getNode(path);
    return node && this.current.slice(node.start, node.end);
  }

  /** Where a path is written: a map entry's key, else the node's first character. */
  positionOf(path: K7Path): { line: number; col: number } | undefined {
    const parent = path.length > 0 ? this.getNode(path.slice(0, -1)) : null;
    const entry =
      parent?.kind === "map"
        ? parent.entries.find((e) => e.key === path[path.length - 1])
        : undefined;
    const at = entry?.keyStart ?? this.getNode(path)?.start;
    return at === undefined ? undefined : lineCol(this.current, at);
  }

  /** A view whose every path is relative to `base`. */
  at(base: K7Path): K7Scope {
    return new K7Scope(this, base);
  }

  /** Runs one edit; the text after it, its splices, and its own answer, or its failure thrown. */
  private run(op: EditOp): boolean | null {
    const a = editText(this.current, [op]);
    if (a.text !== this.current) {
      this.current = a.text;
      this.data = null;
      this.syntax = null;
      this.tree = null;
    }
    this.changes.push(...a.changes);
    if (a.error)
      throw a.error.syntax
        ? syntaxError(a.error.syntax)
        : new Error(a.error.message);
    return a.results[0] ?? null;
  }

  /** Sets the value at a path, creating the maps on the way; a value equal to the one there changes nothing. */
  setIn(path: K7Path, value: unknown, opts?: WriteOptions): void {
    this.run({
      op: "setIn",
      path: [...path],
      value: toWire(value),
      inline: opts?.inline,
    });
  }

  /** Sets (or creates) the value at a path as k7x source, which must read as one value; throws when it does not. */
  setSourceIn(path: K7Path, source: string): void {
    this.run({ op: "setSourceIn", path: [...path], source });
  }

  /** Appends to the list at a path, creating it when it is missing. */
  addIn(path: K7Path, value: unknown, opts?: WriteOptions): void {
    this.run({
      op: "addIn",
      path: [...path],
      value: toWire(value),
      inline: opts?.inline,
    });
  }

  /** Replaces the value at a path with `source` as written, k7x text that must read back; throws when nothing is there. */
  replaceSource(path: K7Path, source: string): void {
    this.run({ op: "replaceSource", path: [...path], source });
  }

  /** An index the Rust side reads as a usize: anything else is refused here, as the editor refused it. */
  private checkIndex(path: K7Path, index: number): void {
    if (Number.isInteger(index) && index >= 0) return;
    if (this.getNode(path)?.kind !== "list")
      throw new Error(`k7x: ${path.join(".")} is not a list`);
    throw new Error(`k7x: no index ${index} to insert at`);
  }

  /** Inserts into the list at a path before item `index`, on a line of its own when that item has one. */
  insertIn(
    path: K7Path,
    index: number,
    value: unknown,
    opts?: WriteOptions,
  ): void {
    this.checkIndex(path, index);
    this.run({
      op: "insertIn",
      path: [...path],
      index,
      value: toWire(value),
      inline: opts?.inline,
    });
  }

  /** Inserts k7x source into the list at a path before item `index`; throws when it does not read as one value. */
  insertSourceIn(path: K7Path, index: number, source: string): void {
    this.checkIndex(path, index);
    this.run({ op: "insertSourceIn", path: [...path], index, source });
  }

  /** Moves a list item so it ends up at index `to`, its source kept as written. */
  moveIn(path: K7Path, from: number, to: number): void {
    const list = this.getNode(path);
    if (list?.kind !== "list")
      throw new Error(`k7x: ${path.join(".")} is not a list`);
    const n = list.items.length;
    const ok = (i: number) => Number.isInteger(i) && i >= 0 && i < n;
    if (!ok(from) || !ok(to))
      throw new Error(`k7x: no move from ${from} to ${to} in ${n} items`);
    this.run({ op: "moveIn", path: [...path], from, to });
  }

  /** Renames the entry at a path, its value, comments and place kept; throws when the map already has `newKey`. */
  renameKey(path: K7Path, newKey: string): void {
    this.run({ op: "renameKey", path: [...path], key: newKey });
  }

  /** Makes the data at a path equal `value` with the fewest edits; [] is the whole file. */
  applyIn(path: K7Path, value: unknown): void {
    this.run({ op: "applyIn", path: [...path], value: toWire(value) });
  }

  /** Removes the entry or item at a path; `prune` also removes the maps that leaves empty, up to a list or the root. */
  deleteIn(path: K7Path, opts?: { prune?: boolean }): boolean {
    return (
      this.run({ op: "deleteIn", path: [...path], prune: opts?.prune }) === true
    );
  }

  /** The `//` lines the file starts with, each without `//` and one space. */
  get header(): string[] {
    const t = this.current;
    const lines: string[] = [];
    let pos = t.startsWith(BOM) ? 1 : 0;
    while (t.startsWith("//", pos)) {
      const nl = t.indexOf("\n", pos);
      const end = nl < 0 ? t.length : nl;
      lines.push(
        t
          .slice(pos + 2, end)
          .replace(/\r$/, "")
          .replace(/^ /, ""),
      );
      pos = nl < 0 ? t.length : nl + 1;
    }
    return lines;
  }

  /** Replaces the file's header with these lines; an empty array removes it. */
  setHeader(lines: readonly string[]): void {
    this.run({ op: "setHeader", lines: [...lines] });
  }
}

/** A view of a document under one base path: it holds a path, resolved again on every call, so it survives every edit. */
export class K7Scope {
  constructor(
    readonly doc: K7Document,
    readonly base: K7Path,
  ) {}

  private at(path: K7Path): K7Path {
    return [...this.base, ...path];
  }

  getIn(path: K7Path): unknown {
    return this.doc.getIn(this.at(path));
  }

  hasIn(path: K7Path): boolean {
    return this.doc.hasIn(this.at(path));
  }

  getNode(path: K7Path): K7Node | undefined {
    return this.doc.getNode(this.at(path));
  }

  getSource(path: K7Path): string | undefined {
    return this.doc.getSource(this.at(path));
  }

  toJS(): unknown {
    return this.doc.getIn(this.base);
  }

  setIn(path: K7Path, value: unknown, opts?: WriteOptions): void {
    this.doc.setIn(this.at(path), value, opts);
  }

  addIn(path: K7Path, value: unknown, opts?: WriteOptions): void {
    this.doc.addIn(this.at(path), value, opts);
  }

  insertIn(
    path: K7Path,
    index: number,
    value: unknown,
    opts?: WriteOptions,
  ): void {
    this.doc.insertIn(this.at(path), index, value, opts);
  }

  deleteIn(path: K7Path, opts?: { prune?: boolean }): boolean {
    return this.doc.deleteIn(this.at(path), opts);
  }

  applyIn(path: K7Path, value: unknown): void {
    this.doc.applyIn(this.at(path), value);
  }

  renameKey(path: K7Path, newKey: string): void {
    this.doc.renameKey(this.at(path), newKey);
  }

  setSourceIn(path: K7Path, source: string): void {
    this.doc.setSourceIn(this.at(path), source);
  }

  insertSourceIn(path: K7Path, index: number, source: string): void {
    this.doc.insertSourceIn(this.at(path), index, source);
  }

  moveIn(path: K7Path, from: number, to: number): void {
    this.doc.moveIn(this.at(path), from, to);
  }

  positionOf(path: K7Path): { line: number; col: number } | undefined {
    return this.doc.positionOf(this.at(path));
  }

  /** A narrower view, its base relative to this one. */
  scope(path: K7Path): K7Scope {
    return new K7Scope(this.doc, this.at(path));
  }
}

export const parseDocument = (text: string): K7Document => new K7Document(text);
