// tree.ts — a k7x text's tree, every node with its UTF-16 span, built from the `k7x.outline` op's flat cells.
// A reader would get wrong that the module sends a tree: it sends numbers and one JSON array (k7x's outline.rs); the nodes are made here.

import { call, outlineCells } from "../abi.ts";
import type { ReadAnswer } from "../types/index.ts";
import { type K7Path, syntaxError } from "./read.ts";

export type K7Scalar = string | number | boolean | null;

export interface K7ScalarNode {
  kind: "scalar";
  value: K7Scalar;
  start: number;
  end: number;
}

export interface K7ListNode {
  kind: "list";
  items: K7Node[];
  /** `brackets` is `[a, b]`; `args` is a call's own argument list, `key(a, b)`. */
  form: "brackets" | "args";
  start: number;
  end: number;
}

export interface K7Entry {
  key: string;
  value: K7Node;
  /** From the key (or the `.` before it, in a chain) to the end of the value (or the `)` of the call). */
  start: number;
  end: number;
  /** Offset of the `.` that introduces a chained call; null for a chain's head and for an object entry. */
  dot: number | null;
  keyStart: number;
  /** Just past the key's source, quotes included. */
  keyEnd: number;
}

export interface K7MapNode {
  kind: "map";
  entries: K7Entry[];
  /** `root` is the file itself, `chain` is `a(1).b(2)`, `object` is `{ a: 1 }`. */
  form: "root" | "chain" | "object";
  /** For the root, each statement's entries as [first, end) indices: a statement is one chain. */
  groups: [number, number][];
  start: number;
  end: number;
}

export type K7Node = K7ScalarNode | K7ListNode | K7MapNode;

type Value = string | number | boolean | null;
const MAP_FORMS = ["root", "chain", "object"] as const;
/** The dot cell of an entry that has none (outline.rs's NO_DOT). */
const NO_DOT = 0xffffffff;

/** The cells and values of outline.rs back into nodes, each read in the order it was written. */
class Reader {
  at = 0;
  v = 0;
  readonly c: Uint32Array;
  readonly values: Value[];
  constructor(c: Uint32Array, values: Value[]) {
    this.c = c;
    this.values = values;
  }

  node(): K7Node {
    const c = this.c;
    const tag = c[this.at++];
    const start = c[this.at++];
    const end = c[this.at++];
    if (tag === 0)
      return { kind: "scalar", value: this.values[this.v++], start, end };
    if (tag <= 2) {
      const n = c[this.at++];
      const items: K7Node[] = new Array(n);
      for (let i = 0; i < n; i++) items[i] = this.node();
      return {
        kind: "list",
        form: tag === 1 ? "brackets" : "args",
        items,
        start,
        end,
      };
    }
    const n = c[this.at++];
    const entries: K7Entry[] = new Array(n);
    for (let i = 0; i < n; i++) {
      const key = this.values[this.v++] as string;
      const keyStart = c[this.at++];
      const keyEnd = c[this.at++];
      const eStart = c[this.at++];
      const eEnd = c[this.at++];
      const d = c[this.at++];
      const value = this.node();
      entries[i] = {
        key,
        value,
        start: eStart,
        end: eEnd,
        dot: d === NO_DOT ? null : d,
        keyStart,
        keyEnd,
      };
    }
    const g = c[this.at++];
    const groups: [number, number][] = new Array(g);
    for (let i = 0; i < g; i++) {
      const first = c[this.at++];
      groups[i] = [first, c[this.at++]];
    }
    return {
      kind: "map",
      form: MAP_FORMS[tag - 3],
      entries,
      groups,
      start,
      end,
    };
  }
}

/** The tree of a whole file; throws K7SyntaxError. An empty file is an empty map. */
export function parseTree(text: string): K7MapNode {
  const a = call<ReadAnswer>({ op: "k7x.outline", args: { text } });
  if (a.syntax) throw syntaxError(a.syntax);
  // Read at once: the view dies at the next call.
  return new Reader(outlineCells(), a.values ?? []).node() as K7MapNode;
}

/** A list index from a path segment: a number, or a string of digits ("2" is 2); else undefined. */
export function indexOf(seg: string | number): number | undefined {
  if (typeof seg === "number") return seg;
  return /^(?:0|[1-9]\d*)$/.test(seg) ? Number(seg) : undefined;
}

/** A node's child at one path segment: a map entry's value by key, a list item by index. */
export function childAt(
  node: K7Node,
  seg: string | number,
): K7Node | undefined {
  if (node.kind === "map")
    return typeof seg === "string"
      ? node.entries.find((e) => e.key === seg)?.value
      : undefined;
  if (node.kind === "list") {
    const i = indexOf(seg);
    return i === undefined ? undefined : node.items[i];
  }
  return undefined;
}

/** The node at a path, or undefined: a string segment names a map entry, a number (or a string of digits) a list item. */
export function nodeAt(root: K7Node, path: K7Path): K7Node | undefined {
  let node: K7Node | undefined = root;
  for (const seg of path) {
    if (node === undefined) return undefined;
    node = childAt(node, seg);
  }
  return node;
}

/** Visits every node depth-first, parents first; `visit` returning false skips that node's children. */
export function walk(
  root: K7Node,
  // biome-ignore lint/suspicious/noConfusingVoidType: a visitor that returns nothing must type-check, and false is its one signal.
  visit: (node: K7Node, path: K7Path, entry?: K7Entry) => void | false,
): void {
  const go = (node: K7Node, path: K7Path, entry?: K7Entry): void => {
    if (visit(node, path, entry) === false) return;
    if (node.kind === "list")
      for (let i = 0; i < node.items.length; i++)
        go(node.items[i], [...path, i]);
    else if (node.kind === "map")
      for (const e of node.entries) go(e.value, [...path, e.key], e);
  };
  go(root, []);
}
