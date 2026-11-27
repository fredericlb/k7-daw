// clipFiles.ts — putting bytes into a project's `clips/` and declaring them. Two writes, shared by
// the timeline's file DROP and by a recorded picture TAKE, which do the same thing for different
// reasons and must not drift on the one that matters: an existing declaration is never overwritten.
// A name already taken by DIFFERENT bytes is not reused: the clip gets the next free id. See
// apps/k7-daw-frontend/docs/timeline.md, "Same name, different file".

import {
  candidateIds,
  clipDecl,
  declaredSource,
  MAX_SUFFIX,
} from "@studio/k7-wasm/intake";
import { projectDir } from "@studio/k7-wasm/paths";
import { mediaUrl } from "@studio/project-shell/fileApi";
import { api } from "./api";

/** What filing one clip did — the id it was filed under, and `reused` when that clip was already there with these bytes. */
export interface FiledClip {
  id: string;
  reused: boolean;
}

/** How many bytes a drop or a take carries, whichever form it arrives in. */
const sizeOfBytes = (b: Blob | ArrayBuffer): number =>
  b instanceof Blob ? b.size : b.byteLength;

/** The size of a file already in the project, or null when it is not there or the service will not say. */
async function sizeOf(path: string): Promise<number | null> {
  try {
    const res = await fetch(mediaUrl(path), { method: "HEAD" });
    if (!res.ok) return null;
    const n = Number(res.headers.get("content-length"));
    return Number.isFinite(n) && n >= 0 ? n : null;
  } catch {
    return null;
  }
}

/** File `upload` under the first id whose existing file, if any, is these bytes — a reused declaration is KEPT, trims and all. */
export async function fileClip(
  project: string,
  clip: { id: string; upload: string },
  bytes: Blob | ArrayBuffer,
  onProgress?: (loaded: number) => void,
): Promise<FiledClip> {
  const dir = projectDir(project);
  const prefix = dir ? `${dir}/` : "";
  const dot = clip.upload.lastIndexOf(".");
  const ext = dot > 0 ? clip.upload.slice(dot) : "";
  // Same size is taken as same file: a re-drop of one recording must not pile up copies of it.
  const same = async (upload: string) =>
    (await sizeOf(`${prefix}clips/${upload}`)) === sizeOfBytes(bytes);

  for (const id of candidateIds(clip.id)) {
    const upload = `${id}${ext}`;
    const decl = `${prefix}clips/${id}.k7x`;
    const existing = await api.read(decl).then(
      (r) => r.content,
      (e: { status?: number }) => {
        if (e.status === 404) return null;
        throw e;
      },
    );
    if (existing !== null) {
      // Declared already: reused only when it names this file and the file is these bytes.
      if (
        declaredSource(existing) === `clips/${upload}` &&
        (await same(upload))
      )
        return { id, reused: true };
      continue;
    }
    try {
      await api.uploadWithProgress(`${prefix}clips`, upload, bytes, onProgress);
    } catch (e) {
      const err = e as Error & { status?: number };
      if (err.status !== 409) throw err;
      // Bytes there with no declaration: they are ours only when they are the same.
      if (!(await same(upload))) continue;
    }
    await api.write(decl, clipDecl(upload));
    return { id, reused: false };
  }
  throw new Error(
    `clips/ already holds ${MAX_SUFFIX} different files called ${clip.id}`,
  );
}
