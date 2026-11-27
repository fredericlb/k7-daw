// index.ts — k7x, the Strudel-looking text of a workspace's files, read, printed and edited by the Rust crate through the module.
// A reader would get wrong that any of it waits: every call is sync and throws `unavailable` before wasmReady() resolves.

export type { EditOp } from "../types/index.ts";
export {
  editText,
  type K7Change,
  K7Document,
  K7Scope,
  parseDocument,
  type WriteOptions,
} from "./document.ts";
export {
  type K7Path,
  K7SyntaxError,
  lineCol,
  parse,
  type StringifyOptions,
  stringify,
} from "./read.ts";
export {
  childAt,
  indexOf,
  type K7Entry,
  type K7ListNode,
  type K7MapNode,
  type K7Node,
  type K7Scalar,
  type K7ScalarNode,
  nodeAt,
  parseTree,
  walk,
} from "./tree.ts";
