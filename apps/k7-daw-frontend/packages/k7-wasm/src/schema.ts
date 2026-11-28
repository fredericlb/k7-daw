// schema.ts — the small rules of a name, a ref, a clip or a key, answered by the Rust crates (ops/schema.rs).
// A reader would get wrong that these wait for the module: they are sync, and throw `unavailable` before wasmReady() resolves.
// A value a loop asks of many inputs has a batch form (`beatsAtSources`): one call, not one per sample.

import { call } from "./abi.ts";
import { DOC_EXT_SRC, IMAGE_EXTS } from "./consts.generated.ts";
import type { BuiltinPluginInfo, EngineModule, Signal } from "./types/index.ts";

export * from "./ops/schema.ts";

export type {
  BuiltinParam,
  BuiltinPluginInfo,
  CompSpan,
  EngineModule,
  Key,
  Playlist,
  PlaylistItem,
  Signal,
  Task,
  TimelineLane,
} from "./types/index.ts";

/** One point of a stretch map: where a moment of the source lands on the timeline, `[beat, second]`. */
export type WarpMarker = [beat: number, sec: number];

const ask = <R>(op: string, args: object): R =>
  call<R>({ op: `schema.${op}`, args });

/** The signal a module of this type folder emits, from its io role or its ports; undefined for a sink. */
export const moduleEmits = (
  category: string | undefined,
  io?: string,
  outputs?: readonly { type: Signal }[],
): Signal | undefined =>
  ask<Signal | null>("module_emits", { category, io, outputs }) ?? undefined;

/** A module's `status:` as the completions treat it: `listed` for anything that does not say otherwise. */
export const moduleStatusOf = (
  m: unknown,
): "listed" | "favorite" | "hidden" => {
  const s = (m as { status?: unknown } | null | undefined)?.status;
  return ask("module_status_of", { status: typeof s === "string" ? s : null });
};

/** How a scanned plugin becomes a module, null when its name leaves no slug. */
export const engineModuleOf = (p: {
  name: string;
  format: string;
}): EngineModule | null =>
  ask("engine_module_of", {
    name: p.name ?? "",
    format: String(p.format ?? ""),
  });

/** The built-in behind an `extends:` ref, the engine namespace only. */
export const builtinPluginForRef = (
  ref: string | null | undefined,
): BuiltinPluginInfo | undefined =>
  ref
    ? (ask<BuiltinPluginInfo | null>("builtin_for_ref", { ref }) ?? undefined)
    : undefined;

/** The built-in a module's `plugin:` declares (`type: internal`, any spelling of the name), or undefined. */
export const builtinPluginFor = (
  plugin: { type?: unknown; name?: unknown } | null | undefined,
): BuiltinPluginInfo | undefined =>
  typeof plugin?.type === "string" && typeof plugin.name === "string"
    ? (ask<BuiltinPluginInfo | null>("builtin_for_plugin", {
        type: plugin.type,
        name: plugin.name,
      }) ?? undefined)
    : undefined;

/** A regex matching `prefix` (pattern source) followed by the document extension at the end. */
export const docPattern = (prefix: string, flags = "i"): RegExp =>
  new RegExp(`${prefix}${DOC_EXT_SRC}$`, flags);

const ending = (exts: readonly string[]): RegExp =>
  new RegExp(`\\.(${exts.join("|")})$`, "i");

/** A clip is a STILL when its source ends in one of the image extensions. */
export const IMAGE_SOURCE = ending(IMAGE_EXTS);

/** The first free `<owner>_<what>_<n>`, counting from 1: `taken` lists the names already used. */
export const instanceName = (
  what: string,
  owner: string | null | undefined,
  taken: Iterable<string>,
): string =>
  ask("instance_name", { what, owner: owner ?? null, taken: [...taken] });

/** The timeline beats that source seconds land on, in one call; extrapolates along the end segments. */
const beatsAtSources = (
  markers: readonly WarpMarker[],
  secs: readonly number[],
): number[] => ask("beats_at_sources", { markers, secs });

/** The timeline beat a source second lands on. */
export const beatAtSource = (
  markers: readonly WarpMarker[],
  sec: number,
): number => beatsAtSources(markers, [sec])[0];
