// drawers.ts — the surface drawers are registered by `plugins.ts`, which rides with the editor's chunk: a page that
// asks which types a device can draw (the Surfaces tab, a remote view) loads it on first need, once.

let registered: Promise<void> | null = null;

/** Resolves once the shell's drawers are in the registry (`deviceSurfaceDrawer`). */
export function drawersReady(): Promise<void> {
  registered ??= import("../plugins").then(() => undefined);
  return registered;
}
