// renderTypes.ts — the Renderer's shape: types only, so a browser bundle imports it for free.

import type { ProjectDoc } from "@studio/song/project";

/** The engine's render manifest: the mixdown plus, with `stems`, one wav per audible track, relative to the folder. */
export interface RenderManifest {
  mix: string;
  stems: { name: string; file: string }[];
}

/** How to bounce: per-track stems beside the mix, and the folder relative clip paths resolve under. */
export interface RenderOptions {
  stems?: boolean;
  base?: string;
}

/** Bounce `doc` into `outDir`, which exists; throws why it could not. */
export type Renderer = (
  doc: ProjectDoc,
  outDir: string,
  opts?: RenderOptions,
) => Promise<RenderManifest>;
