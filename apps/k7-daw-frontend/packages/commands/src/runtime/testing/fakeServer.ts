// fakeServer.ts — a Remote over a real folder, standing in for the server: the bundle, files by name, and a change set it lands or refuses.
// What a reader gets wrong: it models the 409 rules of file-changes.md (the token and each `was`), not the journal or the relay.

import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { textHash } from "@studio/k7-wasm/engine";
import type {
  ChangeSet,
  ChangesAnswer,
  Fetched,
  HostAnswer,
  JobState,
  LibraryFiles,
  Remote,
  VerbBundle,
} from "../remote.ts";
import { withVfs } from "../shims/current.ts";
import type { Vfs } from "../vfs.ts";
import { diskBundle, diskLibrary, spellingOf } from "./diskBundle.ts";

export interface FakeServer extends Remote {
  /** Every change set it was sent, in order, with its answer's status. */
  readonly sets: { set: ChangeSet; status: number }[];
  /** How many times each route answered. */
  readonly calls: {
    bundle: number;
    library: number;
    file: number;
    changes: number;
  };
  /** Runs once, just before the next change set is judged: a peer's write landing under the Worker. */
  beforeNextChange(fn: () => void): void;
  /** The project's token now. */
  token(): string;
  /** Answers a host route (`render`, `probe`…) the test's way; a route nobody serves is a 503 `off`. */
  serve(
    path: string,
    fn: (
      body: Record<string, unknown> | null,
      query: string,
    ) => HostAnswer | Promise<HostAnswer>,
  ): void;
  /** A job that answers each state in turn and then the last one forever; its id. */
  startJob(states: JobState[]): string;
  /** Holds a job's output where `adopt` finds it, as the server's `.job-out` does. */
  stage(job: string, out: string, text: string): void;
}

function walkTexts(dir: string, into: string[]): void {
  let names: string[] = [];
  try {
    names = readdirSync(dir);
  } catch {
    return;
  }
  for (const n of names.sort()) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) walkTexts(p, into);
    else into.push(`${p}\0${readFileSync(p, "latin1")}`);
  }
}

export function fakeServer(
  disk: Vfs,
  workspace: string,
  project: string,
): FakeServer {
  const dir = join(workspace, project);
  const sets: FakeServer["sets"] = [];
  const calls = { bundle: 0, library: 0, file: 0, changes: 0 };
  let hook: (() => void) | undefined;
  const routes = new Map<string, Parameters<FakeServer["serve"]>[1]>();
  const jobs = new Map<string, { states: JobState[]; at: number }>();
  const staged = new Map<string, string>();
  const onDisk = <T>(f: () => T): T => withVfs(disk, f);
  const token = (): string =>
    onDisk(() => {
      const all: string[] = [];
      walkTexts(dir, all);
      return createHash("sha1")
        .update(all.join("\n"))
        .digest("hex")
        .slice(0, 12);
    });
  return {
    sets,
    calls,
    token,
    beforeNextChange: (fn) => {
      hook = fn;
    },
    serve: (path, fn) => void routes.set(path, fn),
    startJob(states) {
      const id = `fake${jobs.size.toString(16).padStart(28, "0")}`;
      jobs.set(id, { states, at: 0 });
      return id;
    },
    stage: (job, out, text) => void staged.set(`${job}/${out}`, text),
    async host(path, body, query = ""): Promise<HostAnswer> {
      const fn = routes.get(path);
      if (!fn)
        return {
          status: 503,
          body: { error: "this host member is not served here", code: "off" },
        };
      return fn(body, query);
    },
    async envelope(clip) {
      const a = await routes.get("waveform")?.({ clip }, "");
      const body = a?.body as { rms?: number[]; seconds?: number } | undefined;
      return a?.status === 200 && body?.rms && typeof body.seconds === "number"
        ? { rms: body.rms, seconds: body.seconds }
        : null;
    },
    async job(id): Promise<JobState> {
      const j = jobs.get(id);
      if (!j) return { state: "failed", error: "no such job", code: "gone" };
      const s = j.states[Math.min(j.at, j.states.length - 1)];
      j.at++;
      return s ?? { state: "failed", error: "no such job", code: "gone" };
    },
    async bundle(at): Promise<VerbBundle> {
      calls.bundle++;
      if (at !== project) throw new Error(`'${at}' is not the open project`);
      return onDisk(() => diskBundle(dir, token()));
    },
    async library(_at, name): Promise<LibraryFiles> {
      calls.library++;
      return onDisk(() => {
        const root = spellingOf(dir).real(`/l/${name}`);
        if (root === null) throw new Error(`no library '${name}'`);
        return diskLibrary(name, root);
      });
    },
    async file(rel): Promise<Fetched> {
      calls.file++;
      return onDisk(() => {
        const p = join(workspace, rel);
        return existsSync(p) ? { text: readFileSync(p, "utf8") } : null;
      });
    },
    async changes(set): Promise<ChangesAnswer> {
      calls.changes++;
      if (hook) {
        const h = hook;
        hook = undefined;
        onDisk(h);
      }
      const answer = onDisk(() => judge(set));
      sets.push({ set, status: answer.status });
      return answer;
    },
  };

  function judge(set: ChangeSet): ChangesAnswer {
    if (set.at !== project)
      return {
        status: 409,
        body: {
          error: "not the open project",
          open: project,
          project: false,
          stale: [],
          rev: token(),
        },
      };
    const now = token();
    const stale: string[] = [];
    const text = (rel: string): string | null => {
      const p = join(workspace, rel);
      return existsSync(p) ? readFileSync(p, "utf8") : null;
    };
    for (const op of set.ops) {
      if (op.kind === "adopt" && !staged.has(`${op.job}/${op.out}`))
        return {
          status: 400,
          body: { error: `'${op.job}/${op.out}' names no staged output` },
        };
    }
    for (const op of set.ops) {
      if (
        (op.kind === "text" || op.kind === "remove" || op.kind === "read") &&
        op.was !== undefined
      ) {
        const t = text(op.file);
        const here = t === null ? null : textHash(t);
        if (here !== op.was && !(op.kind === "text" && t === op.text))
          stale.push(op.file);
      }
    }
    if (stale.length || set.base !== now)
      return {
        status: 409,
        body: { error: "stale", stale, project: set.base !== now, rev: now },
      };
    const changed: { path: string; kind: string }[] = [];
    for (const op of set.ops) {
      const p = join(workspace, op.file);
      if (op.kind === "text") {
        const existed = existsSync(p);
        if (existed && readFileSync(p, "utf8") === op.text) continue;
        mkdirSync(dirname(p), { recursive: true });
        writeFileSync(p, op.text ?? "");
        changed.push({
          path: op.file.slice(project.length + 1),
          kind: existed ? "edited" : "added",
        });
      } else if (op.kind === "remove") {
        rmSync(p, { force: true });
        changed.push({
          path: op.file.slice(project.length + 1),
          kind: "removed",
        });
      } else if (op.kind === "adopt") {
        mkdirSync(dirname(p), { recursive: true });
        const key = `${op.job}/${op.out}`;
        const existed = existsSync(p);
        writeFileSync(p, staged.get(key) ?? "");
        staged.delete(key);
        changed.push({
          path: op.file.slice(project.length + 1),
          kind: existed ? "edited" : "added",
        });
      } else if (op.kind === "copy" || op.kind === "move") {
        mkdirSync(dirname(p), { recursive: true });
        const from = join(workspace, op.from ?? "");
        if (op.kind === "copy") copyFileSync(from, p);
        else renameSync(from, p);
        changed.push({
          path: op.file.slice(project.length + 1),
          kind: "added",
        });
      }
    }
    return { status: 200, body: { revBefore: now, rev: token(), changed } };
  }
}
