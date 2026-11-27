// schema.ts — the small rules of a name, a ref, a clip or a key, answered by the Rust crates (ops/schema.rs).
// A reader would get wrong that these wait for the module: they are sync, and throw `unavailable` before wasmReady() resolves.
// A value a loop asks of many inputs has a batch form (`beatsAtSources`): one call, not one per sample.

import { call } from "./abi.ts";
import { DOC_EXT_SRC, IMAGE_EXTS } from "./consts.generated.ts";
import type {
  BuiltinPluginInfo,
  CompSpan,
  EngineModule,
  Key,
  Playlist,
  Signal,
  Task,
  TimelineLane,
} from "./types/index.ts";

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

/** Is this ref the engine's: `@engine` or anything under it, never `@engineering`. */
export const isEngineRef = (ref: string): boolean =>
  ask("is_engine_ref", { ref });

/** Can a library be called this? */
export const isLibraryName = (name: string): boolean =>
  ask("is_library_name", { name });

/** Is `path` a file a library's `family` folder holds? */
export const isLibraryFileOf = (family: string, path: string): boolean =>
  ask("is_library_file_of", { family, path });

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

/** Is this a name a preset can have: its file's name, held to what every filesystem agrees on. */
export const isPresetName = (name: string): boolean =>
  ask("is_preset_name", { name });

/** A regex matching `prefix` (pattern source) followed by the document extension at the end. */
export const docPattern = (prefix: string, flags = "i"): RegExp =>
  new RegExp(`${prefix}${DOC_EXT_SRC}$`, flags);

const ending = (exts: readonly string[]): RegExp =>
  new RegExp(`\\.(${exts.join("|")})$`, "i");

/** A clip is a STILL when its source ends in one of the image extensions. */
export const IMAGE_SOURCE = ending(IMAGE_EXTS);

/** A name ending in the document extension `.k7x`, any case. */
export const isDocFile = (name: string): boolean =>
  ask("is_doc_file", { name });

/** `name` without its document extension; any other name unchanged. */
export const stripDocExt = (name: string): string =>
  ask("strip_doc_ext", { name });

/** The first free `<owner>_<what>_<n>`, counting from 1: `taken` lists the names already used. */
export const instanceName = (
  what: string,
  owner: string | null | undefined,
  taken: Iterable<string>,
): string =>
  ask("instance_name", { what, owner: owner ?? null, taken: [...taken] });

/** The value of keys sorted by time at `t`: held before the first and after the last, eased between. */
export const valueAt = (keys: readonly Key[], t: number): number =>
  ask("value_at", { keys, t });

/** The material a placement id names: the last `@` before a number, a repeat's `#n` aside. */
export const materialRefOf = (nestedId: string): string =>
  ask("material_ref_of", { id: nestedId });

/** The timeline and lane an address `<timeline>><lane>` names, or null for a project lane. */
export const timelineLaneOf = (id: string): TimelineLane | null =>
  ask("timeline_lane_of", { id });

/** A workspace playlist's k7x text, or null when it does not read: an unreadable file is not an empty one. */
export const parsePlaylist = (text: string): Playlist | null =>
  ask("parse_playlist", { text });

/** A playlist name as the workspace path holding it, null for one that would leave the folder. */
export const playlistPathOf = (name: string): string | null =>
  ask("playlist_path_of", { name });

/** Every task line of a `TODOS.md`, in order. */
export const parseTasks = (text: string): Task[] =>
  ask("parse_tasks", { text });

/** The comp with `span` laid over it: what it covers is taken from every other span, the base's own dropped. */
export const setCompSpan = (
  comp: readonly CompSpan[],
  span: CompSpan,
  baseRef: string,
): CompSpan[] => ask("set_comp_span", { comp, span, baseRef });

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
