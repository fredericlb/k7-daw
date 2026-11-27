// mixBindings.ts — what a desk binding reads off the engine's LIVE state: is its plugin loaded, what does it show, has the engine caught up.
// A reader would get wrong that the join itself is not here: it is `@studio/k7-wasm/desk`'s (Rust); these lookups read what is on screen.

import type { DeskBinding } from "@studio/k7-wasm/desk";

export type { BindingProblem } from "@studio/k7-wasm/chains";
export type ResolvedBinding = DeskBinding;

type Live = Readonly<Record<string, Record<string, number>>>;

/** Is this binding's target a plugin the engine has LOADED? The DOCUMENT decides, not the graph. */
export function hasPlugin(
  b: ResolvedBinding,
  plugins: ReadonlySet<string>,
): boolean {
  return !!b.pluginId && plugins.has(b.pluginId);
}

/** Does this track leave by an interface pair of its OWN? The CUE control's question. */
export function leavesByItsOwnPair(
  b: ResolvedBinding,
  doc: { tracks: { id: string; outputChannels?: number[] }[] } | null,
): boolean {
  const trackId = b.pluginId?.split(":")[0];
  const track = doc?.tracks.find((t) => t.id === trackId);
  return !!track?.outputChannels?.length;
}

/** Every plugin id in a compiled document: what the engine will have once it is sent. */
export function enginePluginIds(
  doc: { tracks: { plugins: { id: string }[] }[] } | null,
): Set<string> {
  const ids = new Set<string>();
  for (const t of doc?.tracks ?? []) for (const p of t.plugins) ids.add(p.id);
  return ids;
}

/** The binding read BACKWARDS: which control drives this module's param? No-plugin only. */
export function controlDriving(
  bindings: readonly ResolvedBinding[],
  module: string,
  param: string,
): ResolvedBinding | undefined {
  return bindings.find(
    (b) => !b.pluginId && b.module === module && b.param === param,
  );
}

/** What a bound control DRAWS: the hand, else the engine, else nothing. */
export function shownValue(
  binding: ResolvedBinding | undefined,
  held: number | undefined,
  live: Live,
): number | undefined {
  if (held !== undefined) return held;
  if (!binding?.pluginId) return undefined;
  return live[binding.pluginId]?.[binding.param];
}

/** Has the engine caught up, so the override may be dropped? One that never yields has stopped listening. */
export function engineAgrees(
  binding: ResolvedBinding | undefined,
  held: number,
  live: Live,
  epsilon = 0.05,
): boolean {
  const said = binding?.pluginId
    ? live[binding.pluginId]?.[binding.param]
    : undefined;
  return said !== undefined && Math.abs(said - held) < epsilon;
}
