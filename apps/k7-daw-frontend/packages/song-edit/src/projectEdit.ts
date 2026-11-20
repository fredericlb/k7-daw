// projectEdit.ts — changing a PROJECT in its own vocabulary ("set `Filter Freq` on
// module `synth`"): finds the file a name lives in and hands the change to its writer.
// Not yaml/edit.ts (the FILE primitive) — this knows modules, not key paths. Every write
// changes the value and nothing else — comments, key order and layout all survive.

import { join } from "node:path";
import type { K7Scope } from "@studio/k7x/document";
import { editChainItem } from "@studio/song/chainItemEdit";
import { chainFilePath } from "@studio/song/chainProject";
import { stub } from "@studio/song/editStub";
import { loadModules, splitModuleKey } from "@studio/song/loader";
import { sharedRoots } from "@studio/song/moduleFork";
import { moduleFileFor } from "@studio/song/yaml/moduleIO";
import { wsEdit } from "@studio/song/yaml/workspaceDoc";
import type { WarpMarker } from "@studio/song-schema/warpPoints";
import type { OnsetEdits } from "./onsets";

// --- module files -------------------------------------------------------------

// Edit a module's node file in place, resolved through loadModules — so "synth:lead" edits its OWN file.
export function editModuleFile(
  dir: string,
  module: string,
  edit: (doc: K7Scope) => void,
): string {
  return rewriteModule(moduleFileOf(dir, module), edit);
}

/** Where a module's text is: a file of its own, or — `item` set — an item of a chain file. */
interface ModuleText {
  path: string;
  dir?: string;
  item?: string;
}

// Edit a module's document through `edit`, writing only when it changed; the file's path.
function rewriteModule(at: ModuleText, edit: (doc: K7Scope) => void): string {
  if (at.item && at.dir) return editChainItem(at.dir, at.item, edit);
  wsEdit(at.path, (doc) => edit(doc.at([])));
  return at.path;
}

// Where a module's file is — a node's own file in the flat `nodes/`, or a shared one at `@workspace`/`@k7-core`.
function moduleFileOf(dir: string, module: string): ModuleText {
  const modules = loadModules(dir) as Record<
    string,
    { home?: string; chain?: string }
  >;
  const b = modules[module];
  // A chain project's node (not a lane) is an item of its chain file — docs/chainFiles.md.
  if (b?.home === "chains" && b.chain)
    return { path: chainFilePath(dir, b.chain), dir, item: module };
  // A node carries its own folder — the only shape a project's own `loadModules` returns now.
  if (b?.home) {
    const folder = join(dir, b.home);
    const found = moduleFileFor(folder, b.home, module);
    if (!found) throw new Error(`module "${module}" has no file in ${b.home}`);
    return { path: found.path };
  }
  const shared = sharedModuleFileOf(dir, module);
  if (shared) return shared;
  throw new Error(`no module "${module}" in this project`);
}

/** The same for a SHARED catalogue module, whose file lives in the workspace's `_modules` — see docs/projectEdit.md. */
function sharedModuleFileOf(dir: string, key: string): ModuleText | null {
  if (!key.startsWith("@")) return null;
  const { module, node } = splitModuleKey(key);
  const [ns, ...rest] = module.slice(1).split("/");
  const root = sharedRoots(dir).find((r) => r.ns === ns)?.root;
  if (!root || !rest.length) return null;
  // The ref IS the path inside its catalogue — that is how readModuleTree keyed it.
  const folder = join(root, ...rest);
  const found = moduleFileFor(folder, "", node ?? "module");
  return found ? { path: found.path } : null;
}

/** Bind a curve to a module's param — null unbinds, and `trigger` names whose note-ons restart an envelope. */
export function setModuleAutomationBinding(
  dir: string,
  module: string,
  param: string,
  curve: string | null,
  trigger?: string,
): string {
  return stub(
    "song-edit/projectEdit.setModuleAutomationBinding",
    dir,
    module,
    param,
    curve,
    trigger,
  );
}

// Set one param on a module node — on the attribute set that is ON, so a knob turned on set 2 changes set 2.
export function setModuleParam(
  dir: string,
  module: string,
  name: string,
  value: number,
): string {
  return stub("song-edit/projectEdit.setModuleParam", dir, module, name, value);
}

/** What a module's file says its attribute SETS are called, and which is on — read off the FILE, docs/projectEdit.md. */
export function moduleAlternatives(
  dir: string,
  module: string,
): { names: string[]; current: string } {
  return stub("song-edit/projectEdit.moduleAlternatives", dir, module);
}

/** Add one and switch to it — empty, or a copy of the set that is on; a LANE's empty one drops its placements, docs/projectEdit.md. */
export function newModuleAlternative(
  dir: string,
  module: string,
  name?: string,
  opts: { copy?: boolean } = {},
): { path: string; name: string } {
  return stub(
    "song-edit/projectEdit.newModuleAlternative",
    dir,
    module,
    name,
    opts,
  );
}

/** Point a module at its render, or clear it — refused on a SHARED one, see apps/k7-daw-frontend/packages/song/docs/freeze.md. */
export function setModuleFrozen(
  dir: string,
  module: string,
  render: string | null,
): string {
  return stub("song-edit/projectEdit.setModuleFrozen", dir, module, render);
}

/** Set a live input's `monitor` (`off` | `auto` | `on`) — a FIELD rather than a param, docs/projectEdit.md. */
export function setModuleMonitor(
  dir: string,
  module: string,
  value: "off" | "auto" | "on",
): string {
  return stub("song-edit/projectEdit.setModuleMonitor", dir, module, value);
}

/** Choose which SHADER this node runs — a folder under `shaders/`, empty clears it; not checked, docs/projectEdit.md. */
export function setModuleShader(
  dir: string,
  module: string,
  name: string,
): string {
  return stub("song-edit/projectEdit.setModuleShader", dir, module, name);
}

/** Which CAMERA a live picture node opens, by its LABEL, never a per-browser deviceId; empty is the default — apps/k7-daw-frontend/packages/song/docs/camera.md. */
export function setModuleCamera(
  dir: string,
  module: string,
  label: string,
): string {
  return stub("song-edit/projectEdit.setModuleCamera", dir, module, label);
}

/** Choose which MIDI device a midi-in node records from, by its label; empty clears it — any device (apps/k7-daw-frontend/packages/song/docs/midiIn.md). */
export function setModuleDevice(
  dir: string,
  module: string,
  label: string,
): string {
  return stub("song-edit/projectEdit.setModuleDevice", dir, module, label);
}

/** Choose which named BASE this module starts from, per instance — see apps/k7-daw-frontend/packages/song/docs/userPresets.md. */
export function setModuleBase(
  dir: string,
  module: string,
  name: string,
): string {
  return stub("song-edit/projectEdit.setModuleBase", dir, module, name);
}

/** Set a live input's `gain` — the interface trim in dB, 0 being unity, and deliberately unclamped. */
export function setModuleInputGain(
  dir: string,
  module: string,
  db: number,
): string {
  return stub("song-edit/projectEdit.setModuleInputGain", dir, module, db);
}

/** Set an effect's `mix`, the engine's dry/wet around it; 1 (all wet) is no line at all — apps/k7-daw-engine/k7-runtime/docs/mix.md. */
export function setModuleMix(dir: string, module: string, mix: number): string {
  return stub("song-edit/projectEdit.setModuleMix", dir, module, mix);
}

/** Set a module's level into (`input_volume`) or out of it (`output_volume`), in dB; 0 is no line at all — engine docs/mix.md. */
export function setModuleVolume(
  dir: string,
  module: string,
  side: "input" | "output",
  db: number,
): string {
  return stub("song-edit/projectEdit.setModuleVolume", dir, module, side, db);
}

/** Set a live input's `channel` — one for mono, two for a stereo pair the interface really groups, docs/projectEdit.md. */
export function setModuleChannel(
  dir: string,
  module: string,
  channels: number[],
): string {
  return stub("song-edit/projectEdit.setModuleChannel", dir, module, channels);
}

/** Point a file-valued param at one of the module's own assets, ASSETS-RELATIVE — a param like any other, so it lands where setModuleParam's would. */
export function setModuleAsset(
  dir: string,
  module: string,
  param: string,
  file: string | null,
): string {
  return stub("song-edit/projectEdit.setModuleAsset", dir, module, param, file);
}

// --- clip files ---------------------------------------------------------------

/** Write a clip's onset CORRECTIONS — the block that outlives a re-detection; corrections that cancel out remove the key. */
export function setClipOnsets(
  dir: string,
  ref: string,
  edits: OnsetEdits,
): string {
  return stub("song-edit/projectEdit.setClipOnsets", dir, ref, edits);
}

/** Read a clip's `[timelineBeat, sourceSecond]` pairs exactly as the file states them, or null — never an identity map. */
export function clipWarpMarkers(dir: string, ref: string): WarpMarker[] | null {
  return stub("song-edit/projectEdit.clipWarpMarkers", dir, ref);
}

/** Write a clip's stretch map; `algorithm` is only used when the block is CREATED, and the engine ignores it. */
export function setClipWarp(
  dir: string,
  ref: string,
  markers: WarpMarker[],
  algorithm = "elastique",
): string {
  return stub(
    "song-edit/projectEdit.setClipWarp",
    dir,
    ref,
    markers,
    algorithm,
  );
}

/** Where a clip's own file is — exported so the undo journal can back it up BEFORE asking for a write. */
export function clipFilePath(dir: string, ref: string): string {
  return stub("song-edit/projectEdit.clipFilePath", dir, ref);
}

/** A clip's onset corrections as its file holds them — `{}` when it has none. */
export function clipOnsets(dir: string, ref: string): OnsetEdits {
  return stub("song-edit/projectEdit.clipOnsets", dir, ref);
}

/** Repoint a node at another module: rewrite its `extends`, keep everything else — docs/projectEdit.md. */
export function setModuleExtends(
  dir: string,
  module: string,
  ref: string,
): { rel: string; dropped: string[]; alternative?: string } {
  return stub("song-edit/projectEdit.setModuleExtends", dir, module, ref);
}

// --- the graph: graphEdit.ts, re-exported so a caller keeps one import ---

export {
  clearChainNodePositions,
  connectChainNodes,
  deleteChainNode,
  disconnectChainNodes,
  insertChainStep,
  laneOfNode,
  placeChainNode,
  readGraphValue,
  renameChainNode,
  replugChainNodes,
  setBypassEntry,
  setChainNodePosition,
  setChainNodePositions,
  setLaneBypass,
  setModuleBypass,
  unlinkChainNode,
} from "./graphEdit";
