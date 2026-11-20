// import.ts — one import, wherever it runs: pick the mapper for a file, name the new folder, say what was lost.
// What a reader gets wrong: nothing here touches a disk; the Worker's importer reads and writes, this decides (docs/import.md).

import { basename, extname } from "node:path";
import { DROPPABLE_PROJECTS } from "@studio/web-protocol/projects";
import { parseAls, type SampleRef } from "./ableton.js";
import { parseAlsZip } from "./abletonZip.js";
import { parseDawproject } from "./dawproject.js";
import type { EmitStats } from "./emit.js";
import type { Translation } from "./model.js";

/** The foreign project formats that can be translated, by extension. */
export const IMPORTABLE = [".dawproject", ".als", ".zip"] as const;

/** …and the subset a DROP may carry, which the BROWSER also filters on — see `DROPPABLE_PROJECTS`. */
export const DROPPABLE = DROPPABLE_PROJECTS;

/** The most a file of any format may weigh before it is read: the upload's own ceiling (`MAX_UPLOAD_BYTES`). */
export const MAX_IMPORT_BYTES = 512 * 1024 * 1024;

/** What one translation reads: the file, its bytes, and where Live's samples come from. */
export interface ImportInput {
  /** What the log calls the input — a workspace-relative path, or just a filename for a drop. */
  source: string;
  file: string;
  bytes: Uint8Array;
  /** Only a `.als` uses it, and only for the samples it names by path. */
  readSample?: (ref: SampleRef) => Uint8Array | null;
  /** The inflate ceiling of an archive, `MAX_INFLATED_BYTES` when unset. */
  inflateLimit?: number;
}

/** Refuses a size the importer would not hold, before its bytes are read. */
export function checkSize(file: string, size: number): void {
  if (size > MAX_IMPORT_BYTES)
    throw new Error(
      `cannot import "${file}": larger than ${MAX_IMPORT_BYTES / 2 ** 20} MB, the most an import takes`,
    );
}

/** A drop takes the self-contained formats; a lone `.als` names its samples by path and is imported from the tree. */
export function checkDroppable(file: string): void {
  const ext = extname(file).toLowerCase();
  if (!(DROPPABLE as readonly string[]).includes(ext))
    throw new Error(
      `cannot import "${file}": a dropped project must be one of ${DROPPABLE.join(", ")} — ` +
        `a ${ext || "file"} names its samples by path and has to sit beside them, so drop it into the workspace and import it from the tree`,
    );
}

/** The mapper for `input`'s extension, run; the archives inflate no further than `MAX_INFLATED_BYTES`. */
export function translate(input: ImportInput): Translation {
  const { source, file, bytes, readSample, inflateLimit } = input;
  const ext = extname(file).toLowerCase();
  const stem = basename(file, extname(file));
  if (ext === ".als")
    return parseAls(bytes, stem, {
      readSample: readSample ?? (() => null),
      inflateLimit,
    });
  if (ext === ".dawproject") return parseDawproject(bytes, stem, inflateLimit);
  // A zip IS an Ableton project: the folder Collect All and Save leaves, its samples inside it.
  if (ext === ".zip") return parseAlsZip(bytes, stem, inflateLimit);
  throw new Error(
    `cannot import "${source}": expected one of ${IMPORTABLE.join(", ")}`,
  );
}

/**
 * The new project's folder: the name somebody TYPED, refused when taken, or the file's own stem, made free with a suffix.
 * `taken` answers for the workspace's top-level names, which is all a browser holds of it.
 */
export function folderName(
  file: string,
  as: string | undefined,
  taken: (name: string) => boolean,
): string {
  const stem = basename(file, extname(file));
  // A leading `_` is a shared folder's (`_modules`), so a project never starts with one.
  const base =
    (as ?? stem).replace(/[/\\]/g, "_").replace(/^[._]+/, "") || "imported";
  if (as) {
    if (taken(base)) throw new Error(`"${base}" already exists`);
    return base;
  }
  let name = base;
  for (let n = 2; taken(name); n++) name = `${base}-${n}`;
  return name;
}

/** The lines the import says: what was read, what was written and what was lost. */
export function importLog(
  source: string,
  project: string,
  t: Translation,
  emitted: Pick<EmitStats, "standIns">,
  outsideWorkspace: ReadonlySet<string>,
): string[] {
  const { song, stats, warnings } = t;
  return [
    `read ${source}`,
    `project "${song.project.name}" — ${song.project.tempo} bpm, ${song.project.timeSignature}, ${song.project.sampleRate} Hz`,
    `${stats.tracks} tracks · ${stats.audioClips} audio clips · ${stats.midiClips} MIDI clips · ${stats.notes} notes · ${stats.assets} assets`,
    // Said apart from the per-sample warnings because this is the one with a FIX: Collect All and Save.
    ...(outsideWorkspace.size
      ? [
          `warning: ${outsideWorkspace.size} sample(s) live outside the workspace and were not read — in Live, use File > Collect All and Save, then re-import`,
        ]
      : []),
    ...warnings.map((w) => `warning: ${w}`),
    ...(emitted.standIns
      ? [
          `warning: ${emitted.standIns} MIDI track(s) play a STAND-IN instrument — theirs did not cross over; replace it on the graph`,
        ]
      : []),
    `wrote ${project}`,
  ];
}
