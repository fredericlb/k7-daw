// nodeFiles.ts — a node's files on disk: `nodes/<node>.k7x` and, beside it, the `<node>.vstpreset`
// holding its opaque plugin state. What a reader gets wrong: the preset is optional, but wherever
// the node file goes it goes too, or the node would start from the plugin's default.

import { existsSync, renameSync, rmSync } from "node:fs";
import { extname, join } from "node:path";
import { stub } from "./editStub";
import { NODES_DIR } from "./moduleTree";

const PRESET_EXT = ".vstpreset";

/** A node file already found, as `entityFileFor` gives it. */
interface NodeFile {
  path: string;
  rel: string;
}

const presetRel = (node: string): string => `${NODES_DIR}/${node}${PRESET_EXT}`;

/** Move a node's file and its preset to `to`, and the moves as `a → b` lines. */
export function moveNodeFiles(
  dir: string,
  file: NodeFile,
  from: string,
  to: string,
): string[] {
  const rel = `${NODES_DIR}/${to}${extname(file.path)}`;
  const moved = [`${file.rel} → ${rel}`];
  renameSync(file.path, join(dir, rel));
  const preset = join(dir, presetRel(from));
  if (existsSync(preset)) {
    renameSync(preset, join(dir, presetRel(to)));
    moved.push(`${presetRel(from)} → ${presetRel(to)}`);
  }
  return moved;
}

/** Delete a node's file and its preset, and the rel paths removed. */
export function removeNodeFiles(
  dir: string,
  file: NodeFile,
  node: string,
): string[] {
  const removed = [file.rel];
  rmSync(file.path, { force: true });
  const preset = join(dir, presetRel(node));
  if (existsSync(preset)) {
    rmSync(preset, { force: true });
    removed.push(presetRel(node));
  }
  return removed;
}

/** Copy node `from` of `fromDir` (its file at `src`) to node `to` of `dir`, and the rel paths written. */
export function copyNodeFiles(
  src: string,
  fromDir: string,
  from: string,
  dir: string,
  to: string,
): string[] {
  return stub("song/nodeFiles.copyNodeFiles", src, fromDir, from, dir, to);
}
