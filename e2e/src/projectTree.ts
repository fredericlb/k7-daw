// projectTree.ts — a repo folder put into a conformance workspace, as the functional suite seeds its fixtures.
// The one thing a reader gets wrong: text goes through PUT /file and binaries through the upload, which the service tells by extension.
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { expect } from "@playwright/test";
import type { Service } from "./service.ts";

export const REPO = join(import.meta.dirname, "..", "..");

/** Every file under a folder of the repo, relative, for a fixture put into a workspace. */
export function repoFiles(dir: string, rel = ""): string[] {
  return readdirSync(join(dir, rel)).flatMap((name) => {
    const path = rel ? `${rel}/${name}` : name;
    return statSync(join(dir, path)).isDirectory()
      ? repoFiles(dir, path)
      : [path];
  });
}

// The extensions the service takes only through the upload; everything else is text, written through PUT /file.
const BINARY =
  /\.(wav|mp3|ogg|flac|vstpreset|png|ico|jpe?g|mp4|mov|m4v|webm|mkv)$/i;

/** A repo folder written into `ws` under `to`, as the functional suite seeds its fixtures. */
export async function putTree(
  svc: Service,
  ws: string,
  dir: string,
  to = "",
): Promise<void> {
  for (const rel of repoFiles(dir)) {
    const path = to ? `${to}/${rel}` : rel;
    const at = path.lastIndexOf("/");
    const bytes = readFileSync(join(dir, rel));
    const r = BINARY.test(rel)
      ? await svc.upload(
          ws,
          at < 0 ? "." : path.slice(0, at),
          path.slice(at + 1),
          bytes,
        )
      : await svc.put("/file", { path, content: bytes.toString("utf8") }, ws);
    expect(r.status, `${path}: ${JSON.stringify(r.body)}`).toBe(200);
  }
}
