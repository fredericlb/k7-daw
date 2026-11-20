import { stub } from "@studio/song/editStub";
import type {
  NodePresets,
  PresetFile,
  PresetScope,
} from "@studio/song-schema/presets";

/** The module ref a node's presets are keyed by — what the node, as it plays, `extends` — or null for one with none. */
export function presetRefOf(dir: string, node: string): string | null {
  return stub("song-edit/presets.presetRefOf", dir, node);
}

/** The file a preset is, refused when the name would leave its module's folder. */
export function presetFile(
  dir: string,
  scope: PresetScope,
  ref: string,
  name: string,
): string {
  return stub("song-edit/presets.presetFile", dir, scope, ref, name);
}

/** What a node offers: its presets from every scope, workspace first, each scope by name. */
export function listNodePresets(dir: string, node: string): NodePresets {
  return stub("song-edit/presets.listNodePresets", dir, node);
}

/** Write a node's state as a preset — `update` requires it to exist, a new one refuses a taken name. */
export function writeNodePreset(
  dir: string,
  node: string,
  scope: PresetScope,
  name: string,
  mode: "new" | "update",
): { path: string; ref: string; preset: PresetFile } {
  return stub(
    "song-edit/presets.writeNodePreset",
    dir,
    node,
    scope,
    name,
    mode,
  );
}

/** Load a preset onto a node: its engine first, then its values in ONE write, then its kit in place of the node's. */
export function applyPresetToNode(
  dir: string,
  node: string,
  scope: PresetScope,
  name: string,
): {
  params: Record<string, number>;
  switched: boolean;
  rebased: boolean;
  kit: boolean;
  missingEngine?: string;
  missingBase?: string;
} {
  return stub("song-edit/presets.applyPresetToNode", dir, node, scope, name);
}
