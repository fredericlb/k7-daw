// plugin.ts — the K7Plugin contract: what a feature declares, and nothing it doesn't. A plugin
// is a set of panes and modules — each contribution is data that lands in a registry that
// ALREADY existed (the shell's pane ring, the module library). A verb is never a plugin's: it is
// the Rust verbs crate's. Full rationale: docs/sdk.md.

import type { ReactNode } from "react";

/** One surface's drawer, for the shell's pane ring. The PROPS a drawer gets are the shell's to
 *  know — the SDK deliberately does not define them (a drawer is typed where it is MOUNTED, see
 *  docs/sdk.md), so this takes `never`: the bottom type, which any real drawer assigns to. */
export interface PaneDrawerContribution {
  type: string;
  drawer: (props: never) => ReactNode;
}

/** One set of module folders the plugin ships — library modules and dsp-authored ones, served
 *  into the workspace's module library. Data only for now; served on install in a later phase. */
export interface ModuleContribution {
  /** The plugin-qualified ref prefix the folders answer under, e.g. `@transcription/`. */
  namespace: string;
}

export interface K7Plugin {
  /** Stable, namespaced: it prefixes the plugin's module namespaces, so two plugins cannot collide by accident. */
  id: string;
  panes?: readonly PaneDrawerContribution[];
  modules?: readonly ModuleContribution[];
}

/** Identity, for the type it carries: a plugin declares with this so a missing contribution is
 *  a compile error at the declaration, not a missing feature at runtime. */
export const definePlugin = <T extends K7Plugin>(p: T): T => p;
