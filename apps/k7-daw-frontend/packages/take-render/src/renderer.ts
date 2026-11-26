// renderer.ts — WHO bounces a ProjectDoc: a Renderer writes the mix (and the stems) into a folder and
// answers the engine's manifest. The local one runs this machine's engine binary; a host with no
// binary hands in one that asks an engine's machine instead (apps/k7-daw-backend/docs/file-engine-jobs.md).
// A caller never spawns the engine itself, so it does not care which one it was given.

import { spawn } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Renderer, RenderManifest } from "./renderTypes";

export type { Renderer, RenderManifest, RenderOptions } from "./renderTypes";

/** The last stdout line of `--render` is its manifest. */
export function parseManifest(stdout: string): RenderManifest {
  const line = stdout.trim().split(/\r?\n/).filter(Boolean).pop() || "";
  try {
    return JSON.parse(line) as RenderManifest;
  } catch {
    throw new Error(`could not read the engine's render manifest:\n${stdout}`);
  }
}

/** A wedged engine is killed after this: a song bounces in seconds, a long share with stems in minutes. */
const LOCAL_RENDER_TIMEOUT_MS = 10 * 60_000;

/** Render through a SEPARATE, short-lived engine at `engineBin` — never the one serving live playback. */
export function localRenderer(
  engineBin: string,
  timeoutMs = LOCAL_RENDER_TIMEOUT_MS,
): Renderer {
  return async (doc, outDir, opts = {}) => {
    const docFile = join(outDir, "doc.json");
    writeFileSync(docFile, JSON.stringify(doc), "utf8");
    const args = ["--render", docFile, "--out", outDir];
    if (opts.stems) args.push("--stems");
    if (opts.base) args.push("--base", opts.base);
    const stdout = await new Promise<string>((ok, fail) => {
      const child = spawn(engineBin, args, {
        stdio: ["ignore", "pipe", "inherit"],
      });
      let out = "";
      const timer = setTimeout(() => child.kill("SIGKILL"), timeoutMs);
      child.stdout.on("data", (d: Buffer) => {
        out += d.toString("utf8");
      });
      child.on("error", (e) => {
        clearTimeout(timer);
        fail(e);
      });
      child.on("close", (code) => {
        clearTimeout(timer);
        if (code === 0) ok(out);
        else fail(new Error(`engine render failed (exit ${code ?? "?"})`));
      });
    });
    return parseManifest(stdout);
  };
}
