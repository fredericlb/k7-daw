// renderToWav.ts — bounce a doc (or a freeze's upstream) to one wav over a Renderer, with no spawn.
// Pure over the Renderer handed in: the verbs' Worker imports it, so it touches only node:fs, os and path.

import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { freezeCut } from "@studio/song/freeze";
import type { ProjectDoc } from "@studio/song/project";
import type { Renderer } from "./renderTypes";

/** Render `doc`'s mixdown to exactly `outWav`, through a scratch folder of its own. */
export async function renderToWav(
  render: Renderer,
  doc: ProjectDoc,
  outWav: string,
): Promise<void> {
  const tmp = mkdtempSync(join(tmpdir(), "k7-render-"));
  try {
    const manifest = await render(doc, tmp);
    copyFileSync(join(tmp, manifest.mix), outWav);
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
}

/** Bounce a freeze's upstream closure to `outWav` — a MIXDOWN of the plan, see docs/rendering.md. */
export function freezeUpstreamRender(
  render: Renderer,
  doc: ProjectDoc,
  plan: { lanes: Set<string>; modules: Set<string> },
  outWav: string,
): Promise<void> {
  return renderToWav(render, freezeCut(doc, plan), outWav);
}
