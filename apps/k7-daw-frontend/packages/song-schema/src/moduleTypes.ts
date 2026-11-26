// moduleTypes.ts — the module vocabulary, with no filesystem: the type folders and what each
// one means, plus how a module is NAMED. It re-exports its three parts — `metaRefs.ts`,
// `fileParams.ts`, `moduleKeys.ts` — and its tables and rules come from the Rust crates
// (`@studio/k7-wasm/consts` for the names, `@studio/k7-wasm/schema` for the predicates).
//
// A leaf on purpose. These tables are the one thing everything about a module derives
// from — `kind`, `inputs`, `emits`, the ref a `ui.k7x` panel answers to — so knowing what
// `audio/` means costs no import of a loader that reads files, and the engine catalogue
// (which knows about plugins and nothing about folders) can reach them without a cycle.

import { portRef, TRACK_OUT_AUDIO } from "@studio/graph-model";
import {
  ACCOUNT_LIBRARY,
  CUE_NODE,
  IO_SOURCES,
  IO_MODULES as IO_TABLE,
  MIX_NODE,
  type MODULE_STATUSES,
  OUT_NODE,
  RENDER_NODE,
  SHADER_CATEGORY,
  SHAPE_CATEGORY,
  TRANSITION_CATEGORY,
  MODULE_TYPES as TYPE_TABLE,
  VIDEO_OUT_NODE,
} from "@studio/k7-wasm/consts";
import { isLibraryName } from "@studio/k7-wasm/schema";

export {
  ACCOUNT_LIBRARY,
  AUDIO_IN_NODE,
  AUTO_RENDERS_HOME_DIR,
  CAMERA_IO,
  CAMERA_NODE,
  COLD_TYPE,
  CONSOLE_TYPE,
  CUE_NODE,
  DEFAULT_LIBRARY,
  DISSOLVE,
  ENGINE_NS,
  FIXED_NAMESPACES,
  HISTORY_HOME_DIR,
  INPUT_VOLUME_PARAM,
  IO_SOURCES,
  LIBRARIES_DIR,
  LIBRARIES_HOME_DIR,
  MASTER_OWNER,
  MIDI_IN_IO,
  MIDI_IN_NODE,
  MIX_NODE,
  MIX_PARAM,
  MODULE_SOURCES_DIR,
  MODULE_STATUSES,
  NOT_SUPPORTED_TYPE,
  OFFLINE_TYPE,
  OUT_NODE,
  OUTPUT_VOLUME_PARAM,
  OVER,
  RACK_FIRST_NOTE,
  RENDER_NODE,
  SHADER_CATEGORY,
  SHADER_FRAGMENT,
  SHADER_PARAMS,
  SHADER_VERTEX,
  SHADERS_DIR,
  SHAPE_CATEGORY,
  SHAPE_KINDS,
  SHARED_MODULES_DIR,
  TRACK_CATEGORY,
  TRANSITION_CATEGORY,
  VIDEO_OUT_NODE,
  VOLUME_RANGE,
} from "@studio/k7-wasm/consts";
export * from "./fileParams";
export * from "./metaRefs";
export * from "./moduleKeys";

/** The io roles the signal comes FROM; everything else under `io/` is a sink by ELIMINATION — see docs/moduleTypes.md. */
export type IoSource = (typeof IO_SOURCES)[number];
export const isIoSource = (io: unknown): io is IoSource =>
  (IO_SOURCES as readonly unknown[]).includes(io);

/** RENDER's second input: the sound the picture leaves with, the export's audio — apps/k7-daw-frontend/packages/song/docs/videoOutputs.md. */
export const RENDER_AUDIO_PORT = TRACK_OUT_AUDIO;
/** …as an arrow names it, `MIX: ["OUT", "RENDER#audio"]`. */
export const RENDER_AUDIO_IN = `${RENDER_NODE}#${RENDER_AUDIO_PORT}`;

/** The recorder a project with no timeline gets on its master run, by plugin id: what its record button drives — apps/k7-daw-frontend/packages/song/docs/outputRecording.md. */
export const OUTPUT_RECORDER_ID = `${MIX_NODE}:@record`;

/** A compiled plugin's id, `<chain>:<module>`; a module name may hold a colon, so the FIRST one splits. */
export const pluginIdOf = (chain: string, module: string): string =>
  `${chain}:${module}`;
/** The module half of a compiled plugin id — the whole id when it has no colon. */
export const pluginModule = (id: string): string =>
  id.slice(id.indexOf(":") + 1);
/** The chain half of a compiled plugin id. */
export const pluginChain = (id: string): string => id.split(":")[0];

/** The five built-in outputs — the two sums, and the three devices they leave by. */
const BUILTIN_OUTPUTS = [
  MIX_NODE,
  OUT_NODE,
  RENDER_NODE,
  VIDEO_OUT_NODE,
  CUE_NODE,
] as const;
/** Is this name one of them? Neither has a folder on disk, so every "is it a module" test asks. */
export const isBuiltinOutput = (id: string): boolean =>
  (BUILTIN_OUTPUTS as readonly string[]).includes(id);

/** The uniform each shader knob lands on — one rule, so a panel and a shader cannot disagree. */
export const shaderUniformFor = (param: string): string => `u${param}`;

/** The type folders whose modules ARE their behaviour (GLSL, a shape's params), so an empty `plugin:` is no hole. */
const PLUGINLESS_CATEGORIES = [
  SHADER_CATEGORY,
  SHAPE_CATEGORY,
  TRANSITION_CATEGORY,
] as const;

/** …asked the way every consumer wants it: does a module of this type folder hold a PLUGIN at all? */
export const hostsPlugin = (category: string | undefined): boolean =>
  !(PLUGINLESS_CATEGORIES as readonly string[]).includes(category ?? "");

/** One named port on a module, in or out; `mandatory` is an INPUT's business — see docs/moduleTypes.md. */
export interface PortDef {
  name: string;
  type: "audio" | "midi" | "video";
  mandatory?: boolean;
}

/** RENDER's two inputs: the composited picture, and the sound the export leaves with — apps/k7-daw-frontend/packages/song/docs/videoOutputs.md. */
export const RENDER_INPUTS: PortDef[] = [
  { name: "in", type: "video" },
  { name: RENDER_AUDIO_PORT, type: "audio" },
];

/** What a module IS at the coarsest level — four, not two, and the wire treats each differently: docs/moduleTypes.md. */
export type ModuleKind = "io" | "plugin" | "doc" | "track";

/** What a module of each type has when it says nothing itself — its DEFAULT ports, see docs/moduleTypes.md. */
export const MODULE_TYPES: Record<
  string,
  {
    kind: ModuleKind;
    inputs: PortDef[];
    outputs: PortDef[];
  }
> = TYPE_TABLE;

/** The type folder a scanned plugin belongs in — the single answer, since it decides the PORTS: docs/moduleTypes.md. */
export function categoryForRole(p: {
  role?: string;
  instrument?: boolean;
}): string {
  if (p.role === "midiEffect") return "transformers";
  if (p.role === "instrument" || p.instrument === true) return "instruments";
  return "audio";
}

/** How a chain names an engine module: `@engine/neural-amp-modeler`; here so a ref costs no `node:fs` (docs/moduleTypes.md). */
export const engineRef = (slug: string): string => `@engine/${slug}`;

/** How a module asks to be TREATED BY THE COMPLETIONS and nothing else — full rationale: docs/moduleStatus.md. */
export type ModuleStatus = (typeof MODULE_STATUSES)[number];
export const DEFAULT_MODULE_STATUS: ModuleStatus = "listed";

/** A completion list with the hidden dropped and the favourites floated up, STABLE within each group (docs/moduleTypes.md). */
export function completionOrder<T>(
  items: readonly T[],
  statusOf: (item: T) => ModuleStatus,
): T[] {
  const favorite: T[] = [];
  const listed: T[] = [];
  for (const it of items) {
    const s = statusOf(it);
    if (s === "hidden") continue;
    (s === "favorite" ? favorite : listed).push(it);
  }
  return [...favorite, ...listed];
}

/** The io roles the app itself provides — one IN and one OUT for the whole app, and a node carries the wiring: docs/moduleTypes.md. */
export const IO_MODULES: Record<
  string,
  { io: "in" | "out" | "file" | "camera" | "midi" }
> = IO_TABLE;

// --- ports (see docs/ports.md) --------------------------------------------------

// The port vocabulary lives in @studio/graph-model, the one tier both the compiler and the renderer reach (docs/moduleTypes.md).
export { portRef, splitPort } from "@studio/graph-model";

/** A picture lane's SOUND half, `<lane>#audio`, the ref its mute, solo and chains answer to. */
export const laneSoundRef = (lane: string): string =>
  portRef(lane, TRACK_OUT_AUDIO);

/** A module's level: `engine`, `workspace`, or the NAME of a library the account has cloned. */
export type ModuleNamespace = string;

/** `@<library>/<rest>` split into its library and the rest, or null for a ref no library answers to. */
export function libraryRefParts(
  ref: string,
): { library: string; rest: string } | null {
  const m = /^@([^/]+)\/(.+)$/.exec(ref);
  return m && (isLibraryName(m[1]) || m[1] === ACCOUNT_LIBRARY)
    ? { library: m[1], rest: m[2] }
    : null;
}

/** The ref a library gives the thing at `rest` inside it: `@k7-core/audio/reverb`. */
export const libraryRef = (library: string, rest: string): string =>
  `@${library}/${rest}`;
