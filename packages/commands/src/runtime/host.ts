// host.ts — the CommandHost a verb runs against in the browser: the project's compile and the run's own log.
// What a reader gets wrong: a member that reaches the server throws and marks the run, which then never reruns by itself.

import { listBaseModules } from "@studio/song/loader";
import type { ProjectDoc } from "@studio/song/project";
import type { CommandHost } from "../commands.ts";
import { type Compiler, viewOf } from "./compiler.ts";
import { benchMembers } from "./members/bench.ts";
import { deviceMembers } from "./members/devices.ts";
import { engineMembers } from "./members/engine.ts";
import { HostError, type Hosting } from "./members/jobs.ts";
import { libraryMembers } from "./members/library.ts";
import { localMembers } from "./members/local.ts";
import { mediaMembers, takeMember } from "./members/media.ts";
import { pluginMembers } from "./members/plugins.ts";
import { renderMembers } from "./members/render.ts";
import type { Remote } from "./remote.ts";
import { NeedsFiles, type Vfs } from "./vfs.ts";

export type Output = string[] & { refused?: boolean };

/** What a run leaves for the runtime to read after the verb. */
export interface RunFlags {
  /** A verb asked for a reload: the change set says `reload: true`. */
  reload: boolean;
  /** A verb called a member only the server answers; its run cannot be repeated by itself. */
  hostCalled: boolean;
  /** A member the Worker lacks was asked for, whatever the verb did with the throw. */
  unsupported?: string;
}

export interface HostParts {
  vfs: Vfs;
  compiler: Compiler;
  handle: number;
  project: string;
  /** The project's name as `open` was given it: what the server's `at` means. */
  at: string;
  workspaceRoot: string;
  /** The server, for the members that call a host route; `signal` is aborted when the runtime closes. */
  remote: Remote;
  signal: AbortSignal;
  /** The texts last sent to the compiler as drafts, by path, so only what moved crosses again. */
  sent: Map<string, string | null>;
}

/** What a host member needs of this run: the server, the run's flags and the close signal. */
export function hostingOf(p: HostParts, flags: RunFlags): Hosting {
  return { remote: p.remote, flags, signal: p.signal, at: p.at };
}

/** Lays what the run has written over the compiler's bundle. */
export function syncDrafts(p: HostParts): void {
  const files: { path: string; text?: string }[] = [];
  for (const path of p.vfs.touched()) {
    const f = p.vfs.files.get(path);
    const text = f?.text;
    if (f && text === undefined) continue;
    if (p.sent.get(path) === (text ?? null) && p.sent.has(path)) continue;
    p.sent.set(path, text ?? null);
    files.push(text === undefined ? { path } : { path, text });
  }
  if (files.length) p.compiler.update(p.handle, files);
}

/** Puts the compiler back on what the vfs holds, after a rollback or a commit. */
export function resetDrafts(p: HostParts): void {
  const files: { path: string; text?: string }[] = [];
  for (const path of p.sent.keys()) {
    const text = p.vfs.files.get(path)?.text;
    files.push(text === undefined ? { path } : { path, text });
  }
  p.sent.clear();
  if (files.length) p.compiler.update(p.handle, files);
}

/** The module vocabulary a verb can extend, read over the vfs; a project that does not load has none. */
function safeBaseModules(dir: string): string[] {
  try {
    return listBaseModules(dir);
  } catch (e) {
    // A read the bundle lacked must reach the runtime's rerun, and anything else is an empty vocabulary.
    if (e instanceof NeedsFiles) throw e;
    return [];
  }
}

const NEEDS = (name: string) =>
  `${name}: this verb needs the server (3f makes it a route)`;

/** A host for one run: `out` and `flags` are the run's own. */
export function makeBrowserHost(
  p: HostParts,
  out: Output,
  flags: RunFlags,
): CommandHost {
  const server =
    (name: string) =>
    (..._a: unknown[]): never => {
      flags.hostCalled = true;
      flags.unsupported = name;
      throw new HostError(NEEDS(name), "unsupported");
    };
  let baseModules: string[] | undefined;
  const host: Partial<CommandHost> = {
    songPath: p.project,
    project: () => {
      syncDrafts(p);
      return viewOf<{ doc: ProjectDoc }>(
        p.compiler.compile(p.handle, ["engine"]),
        "engine",
      ).doc;
    },
    log: (lines) => out.push(...lines),
    refuse: (lines) => {
      out.refused = true;
      out.push(...lines);
    },
    reload: () => {
      flags.reload = true;
    },
    repoDir: p.workspaceRoot,
    // The bundle states each wav's length, so a verb needing the file's end reads it with no round trip.
    seconds: (file) => p.vfs.secondsOf(file),
    ...renderMembers(hostingOf(p, flags), p.vfs),
    ...mediaMembers(hostingOf(p, flags), p.vfs),
    ...localMembers,
    ...takeMember(
      hostingOf(p, flags),
      () => host.project?.().recordDir ?? "",
      p.project,
    ),
    snapshot: () => {},
    ...engineMembers(hostingOf(p, flags), out),
    ...pluginMembers(hostingOf(p, flags), out),
    ...deviceMembers(hostingOf(p, flags)),
    ...libraryMembers(hostingOf(p, flags)),
    ...benchMembers(hostingOf(p, flags)),
    applyProject: server("applyProject"),
    sendProject: server("sendProject"),
  };
  Object.defineProperty(host, "baseModules", {
    enumerable: true,
    get: () => {
      baseModules ??= safeBaseModules(p.project);
      return baseModules;
    },
  });
  return host as CommandHost;
}
