// fakeWorkspace.ts — a Remote over a folder that stands for a WORKSPACE with no project open: the routes an import calls.
// What a reader gets wrong: it models `create` as landing.rs does (taken, confined, removed when the set does not land), not the journal.

import {
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { textHash } from "@studio/web-protocol/text-hash";
import type {
  ChangeSet,
  ChangesAnswer,
  HostAnswer,
  Remote,
  VerbBundle,
} from "../remote.ts";
import { withVfs } from "../shims/current.ts";
import type { Vfs } from "../vfs.ts";
import { diskBundle } from "./diskBundle.ts";

export interface FakeWorkspace extends Remote {
  /** Every call that reached the server, in order: `upload dir/name`, `changes`, `remove rel`, `bytes rel`. */
  readonly calls: string[];
  /** Every change set it was sent, with its answer's status. */
  readonly sets: { set: ChangeSet; status: number }[];
  /** The binaries uploaded, by workspace-relative path: the disk holds texts only. */
  readonly blobs: Map<string, Uint8Array>;
  /** The engine's plugin list, as `GET /verb-host/plugins` answers it; none is a 503. */
  plugins: unknown[] | undefined;
  /** The next change set fails on the disk with `status`, as a landing failing after its plan passed does. */
  failNextChange(status: number): void;
  /** What the disk holds as text, by workspace-relative path, under `dir`. */
  tree(dir: string): Record<string, string>;
}

const BINARY = /\.(wav|mp3|flac|ogg|aif|aiff|vstpreset|bin)$/i;

/** A workspace at `root` on `disk`, answering as the server's file service does for an empty `at`. */
export function fakeWorkspace(disk: Vfs, root: string): FakeWorkspace {
  const calls: string[] = [];
  const sets: FakeWorkspace["sets"] = [];
  const blobs = new Map<string, Uint8Array>();
  let failing: number | undefined;
  const onDisk = <T>(f: () => T): T => withVfs(disk, f);
  const holdsProject = (dir: string): boolean => {
    if (!existsSync(dir)) return false;
    return readdirSync(dir, { recursive: true }).some((n) =>
      String(n).endsWith(".k7x"),
    );
  };
  const discard = (name: string): void => {
    const dir = join(root, name);
    if (existsSync(dir) && !holdsProject(dir))
      rmSync(dir, { recursive: true, force: true });
    for (const k of [...blobs.keys()])
      if (k.startsWith(`${name}/`)) blobs.delete(k);
  };
  const shared = (rel: string): boolean =>
    rel.startsWith("_modules/") || rel.startsWith("_playlists/");

  const judge = (set: ChangeSet): ChangesAnswer => {
    const name = set.create;
    if (set.discard && name) {
      discard(name);
      return { status: 200, body: {} };
    }
    const refuse = (status: number, error: string): ChangesAnswer => {
      if (name) discard(name);
      return { status, body: { error } };
    };
    if (set.at !== "")
      return {
        status: 400,
        body: { error: "a fake workspace takes an empty at" },
      };
    if (name && holdsProject(join(root, name)))
      return {
        status: 409,
        body: { error: `'${name}' already holds a project`, taken: true },
      };
    for (const op of set.ops) {
      const inside = name !== undefined && op.file.startsWith(`${name}/`);
      if (op.kind !== "read" && !inside && !shared(op.file))
        return refuse(403, `'${op.file}': not a path this set may write`);
    }
    for (const op of set.ops) {
      if (op.kind !== "text" && op.kind !== "read") continue;
      const p = join(root, op.file);
      const was = existsSync(p) ? textHash(readFileSync(p, "utf8")) : null;
      if (op.was !== undefined && op.was !== was && was !== null)
        return refuse(409, `changed since it was read: ${op.file}`);
    }
    if (failing !== undefined) {
      const status = failing;
      failing = undefined;
      return refuse(status, "the write was lost");
    }
    const changed: { path: string; kind: string }[] = [];
    for (const op of set.ops) {
      if (op.kind !== "text") continue;
      const p = join(root, op.file);
      mkdirSync(dirname(p), { recursive: true });
      writeFileSync(p, op.text ?? "");
      changed.push({ path: op.file, kind: "added" });
    }
    return { status: 200, body: { revBefore: "w0", rev: "w1", changed } };
  };

  return {
    calls,
    sets,
    blobs,
    plugins: [],
    failNextChange: (status) => {
      failing = status;
    },
    tree(dir) {
      const out: Record<string, string> = {};
      const walk = (d: string): void => {
        if (!existsSync(d)) return;
        for (const n of readdirSync(d).sort()) {
          const p = join(d, n);
          if (statSync(p).isDirectory()) walk(p);
          else if (!blobs.has(p.slice(root.length + 1)))
            out[p.slice(root.length + 1)] = readFileSync(p, "utf8");
        }
      };
      onDisk(() => walk(join(root, dir)));
      return out;
    },
    async bundle(at): Promise<VerbBundle> {
      if (at !== "") throw new Error(`'${at}' is not the open project`);
      return onDisk(() => {
        const b = diskBundle(join(root, ".no-project"), "w0");
        const lists = b.source.lists;
        const names = existsSync(root)
          ? readdirSync(root).map((name) => ({
              name,
              kind: statSync(join(root, name)).isDirectory()
                ? ("dir" as const)
                : ("file" as const),
            }))
          : [];
        return {
          ...b,
          at: "",
          source: {
            ...b.source,
            lists: { ...lists, [b.workspaceRoot]: names },
          },
        };
      });
    },
    async library() {
      return { name: "x", hash: "x", files: {} };
    },
    async file() {
      return null;
    },
    async bytes(rel) {
      calls.push(`bytes ${rel}`);
      const held = blobs.get(rel);
      if (held) return held;
      return onDisk(() => {
        const p = join(root, rel);
        return existsSync(p)
          ? new TextEncoder().encode(readFileSync(p, "utf8"))
          : null;
      });
    },
    async upload(dir, name, bytes): Promise<HostAnswer> {
      const rel = dir === "." ? name : `${dir}/${name}`;
      calls.push(`upload ${rel}`);
      return onDisk(() => {
        const p = join(root, rel);
        if (existsSync(p))
          return { status: 409, body: { error: `${rel} already exists` } };
        mkdirSync(dirname(p), { recursive: true });
        // The disk is a text disk: a binary leaves a placeholder the listing shows, and its bytes in `blobs`.
        writeFileSync(
          p,
          BINARY.test(name) ? "" : new TextDecoder().decode(bytes),
        );
        blobs.set(rel, bytes);
        return { status: 200, body: { path: rel } };
      });
    },
    async remove(rel): Promise<HostAnswer> {
      calls.push(`remove ${rel}`);
      onDisk(() => rmSync(join(root, rel), { recursive: true, force: true }));
      blobs.delete(rel);
      return { status: 200, body: {} };
    },
    async host(path): Promise<HostAnswer> {
      if (path === "plugins" && this.plugins !== undefined)
        return { status: 200, body: { plugins: this.plugins } };
      return { status: 503, body: { error: "not served here", code: "off" } };
    },
    async job() {
      return { state: "failed", error: "no such job", code: "gone" };
    },
    async changes(set): Promise<ChangesAnswer> {
      calls.push(set.discard ? "discard" : "changes");
      const answer = onDisk(() => judge(set));
      if (!set.discard) sets.push({ set, status: answer.status });
      return answer;
    },
  } as FakeWorkspace;
}
