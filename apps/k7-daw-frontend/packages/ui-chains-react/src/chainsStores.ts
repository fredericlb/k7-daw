// chainsStores.ts — the live values a card reads WITHOUT re-rendering the graph: optimistic
// overrides, MIDI and rack activity, plugin load, the song clock. Each is a
// subscription of the one control that shows it, never of the graph — docs/liveCables.md.

import type { PluginParamsStore } from "@studio/engine-browser/params";
import {
  type EngineStore,
  useEngineStoreOrNull,
} from "@studio/engine-browser/store";
import type { TrackControlValues } from "@studio/ui-chains/graph";
import { createContext, useMemo, useSyncExternalStore } from "react";
import { loadBadge } from "./nodeLoad";

/** The optimistic overrides, held OUTSIDE React state: a drag re-renders the widget it moves, not every card. */
export interface OverrideStore {
  get: () => Record<string, number>;
  set: (next: (o: Record<string, number>) => Record<string, number>) => void;
  subscribe: (cb: () => void) => () => void;
}

export function createOverrideStore(): OverrideStore {
  let map: Record<string, number> = {};
  const subs = new Set<() => void>();
  return {
    get: () => map,
    set: (next) => {
      const out = next(map);
      if (out === map) return;
      map = out;
      for (const cb of subs) cb();
    },
    subscribe: (cb) => {
      subs.add(cb);
      return () => subs.delete(cb);
    },
  };
}

type Subscribe = (cb: () => void) => () => void;
const NO_SUBSCRIBE: Subscribe = () => () => {};

/** One live value off a store that may be absent: a null `subscribe` watches nothing, and `read` answers alone. */
function useStoreSlice<T>(subscribe: Subscribe | null, read: () => T): T {
  return useSyncExternalStore(subscribe ?? NO_SUBSCRIBE, read, read);
}

/** One override, by key — a widget re-renders when ITS value moves; a null key watches nothing. */
export function useOverride(
  store: OverrideStore,
  key: string | null,
): number | undefined {
  return useStoreSlice(store.subscribe, () =>
    key === null ? undefined : store.get()[key],
  );
}

const EMPTY_OVERRIDES: Record<string, number> = {};
const getEmptyOverrides = (): Record<string, number> => EMPTY_OVERRIDES;

/** The whole map, for the one widget that reads every key of its node — an LCD screen; null watches nothing. */
export function useOverrideMap(
  store: OverrideStore | null,
): Record<string, number> {
  return useStoreSlice(
    store ? store.subscribe : null,
    store ? store.get : getEmptyOverrides,
  );
}

/** The override map with one channel dropped — the SAME object when it dropped nothing, docs/widgets.md. */
export function dropChannel(
  o: Record<string, number>,
  prefix: string,
  under: string | undefined,
): Record<string, number> {
  let dropped = false;
  const keep: Record<string, number> = {};
  for (const key in o) {
    if (key.startsWith(prefix) && key !== under) dropped = true;
    else keep[key] = o[key];
  }
  return dropped ? keep : o;
}

export interface Channels {
  // The STORE, not its map, so one plugin's frame re-renders one control — engine-browser's docs/params.md.
  params: PluginParamsStore;
  trackControls: Record<string, TrackControlValues>;
  // A STORE, read per widget by key: the map itself here would re-render every card on every drag step.
  overrides: OverrideStore;
  // The engine's `levels` arrive by POSITION, so metering anything needs the order to name them.
  trackOrder: string[];
  // A module the compiler emits NO plugin for — see `moduleParams` on the props.
  moduleParams: Record<string, Record<string, number>>;
  // Does the host handle those at all? An EMPTY map is a legitimate state — docs/hosting.md.
  offlineChannel: boolean;
  // …and can it TAKE one back? Showing without taking is legitimate — docs/hosting.md.
  offlineWritable: boolean;
  // …and a SHADER's knob, which no surface lends: the host writes it to the node itself (docs/hosting.md).
  shaderWritable: boolean;
  // The same question for the other two channels, asked separately since they answer separately.
  pluginWritable: boolean;
  trackWritable: boolean;
  // …and for a RACK pad, which asks for a sound rather than a value: can this host make one?
  rackTriggerable: boolean;
  projectWritable: boolean;
  // Is this graph a REHEARSAL — there is nowhere to send a value and none is needed.
  rehearsal: boolean;
  /** Which rack sounds are firing right now — a STORE, subscribed per pad. See `RackLitStore`. */
  rackLit?: RackLitStore;
  /** Which MIDI devices just sent something — a midi-in card's LED. See `MidiActivityStore`. */
  midiActivity?: MidiActivityStore;
}

/** Which MIDI devices just sent something, asked by the LABEL a midi-in node names ("" any) — docs/widgets.md. */
export interface MidiActivityStore {
  subscribe(cb: () => void): () => void;
  isLit(label: string): boolean;
}

/** A midi-in card's LED; a null label is "not a midi-in card", so the hook stays unconditional. */
export function useMidiActivity(
  store: MidiActivityStore | undefined,
  label: string | null,
): boolean {
  const subscribe = useMemo(
    () =>
      store && label !== null ? (cb: () => void) => store.subscribe(cb) : null,
    [store, label],
  );
  return useStoreSlice(subscribe, () =>
    store && label !== null ? store.isLit(label) : false,
  );
}

/** Which rack sounds are firing, as a pad asks it: one module, one note — docs/widgets.md. */
export interface RackLitStore {
  subscribeTo(module: string, cb: () => void): () => void;
  isLit(module: string, note: number): boolean;
}

/** One pad's lit state — a null module or note is "nothing to watch", so the hook stays unconditional. */
export function useRackLit(
  store: RackLitStore | undefined,
  module: string | null,
  note: number | null,
): boolean {
  const subscribe = useMemo(
    () =>
      store && module
        ? (cb: () => void) => store.subscribeTo(module, cb)
        : null,
    [store, module],
  );
  return useStoreSlice(subscribe, () =>
    store && module && note != null ? store.isLit(module, note) : false,
  );
}

/** One plugin's cost badge off the store, as a STRING key, so a frame that rounds the same re-renders nothing. */
export function usePluginLoadBadge(
  store: EngineStore | null,
  pluginId: string | null,
): { text: string; title: string } | null {
  const key = useStoreSlice(store ? store.subscribe : null, () => {
    if (!store || !pluginId) return "";
    const b = loadBadge(store.getSnapshot().load?.plugins[pluginId]);
    return b ? `${b.text}\n${b.title}` : "";
  });
  if (!key) return null;
  const [text, title] = key.split("\n");
  return { text, title };
}

/** The song's clock as the host knows it: what turns the engine's seconds into the beats a curve is written in. */
interface SongClock {
  tempo: number;
  num: number;
  den: number;
}
export const SongClockContext = createContext<SongClock | null>(null);
/** Set ONCE by the host around every graph it mounts: without it a curve card reads 4/4 and draws no playhead. */
export const SongClockProvider = SongClockContext.Provider;

/** The song beat the transport is at while it plays, null when stopped — a subscription of the card's picture alone, never of the graph. */
export function usePlayingBeat(tempo: number | undefined): number | null {
  const store = useEngineStoreOrNull();
  return useStoreSlice(store ? store.subscribe : null, () => {
    const s = store?.getSnapshot();
    return s?.playing && tempo ? (s.playhead * tempo) / 60 : null;
  });
}

// A stable empty track order: a fresh `[]` per render would change every Node's props each frame.
export const EMPTY_ORDER: string[] = [];
// …and a stable empty selection, so clearing an empty one re-renders nothing.
export const EMPTY_SET: ReadonlySet<string> = new Set();
export const NOOP = (): void => {};
