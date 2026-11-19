// scaffold.ts — the file map + writer for the two TIMELINE archetypes: `arrangement`, a
// minimal immediately-playable project (one MIDI track through a 4OSC synth + a live
// Strudel pattern), and `video`, the same thing with a lane of PICTURE over it. The third
// archetype, `mix`, is scaffoldSet.ts. A new project is written as chain files.
// Full rationale: docs/scaffold.md, and packages/song-video/docs/picture.md for the video half.

import { copyFileSync, existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { ProjectShape } from "@studio/song-schema/yaml/projectSchema";
import { stub } from "./editStub";
import { forgetModuleReads } from "./moduleCache";

/** The two archetypes with a TIMELINE, by `Extract` so a shape with its own scaffold cannot be passed here. */
export type TimelineShape = Extract<ProjectShape, "arrangement" | "video">;

// The bundled sample: testsrc burns the frame number in, so a sync problem is visible at a glance.
const SAMPLE_VIDEO = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "samples",
  "video",
  "sample.mp4",
); // packages/song/samples/video/sample.mp4

/** Write a project's file map under `dir`, making the folders it implies; a `.k7x` must read, anything else goes as given. */
export function writeProjectFiles(
  dir: string,
  files: Record<string, string>,
): string[] {
  return stub("song/scaffold.writeProjectFiles", dir, files);
}

/** Write a new k7 project; `video` also gets the bundled sample COPIED in — see docs/scaffold.md. */
export function scaffold(
  dir: string,
  name: string,
  shape: TimelineShape = "arrangement",
): string[] {
  const picture = shape === "video";
  if (picture && !existsSync(SAMPLE_VIDEO))
    throw new Error(
      `bundled sample video missing (${SAMPLE_VIDEO}) — the @studio/song package is incomplete`,
    );
  // The sample is bytes and a call carries text: the crate lays an empty file, the copy fills it.
  const written = stub<string[]>(
    "song/scaffold.scaffold",
    dir,
    name,
    shape,
    "",
  );
  if (picture) copyFileSync(SAMPLE_VIDEO, join(dir, "clips/sample.mp4"));
  forgetModuleReads(dir);
  return written;
}
