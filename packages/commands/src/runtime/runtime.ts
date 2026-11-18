// runtime.ts — a verb run where the files are: load the bundle, run over the Vfs, post the writes as one change set.
// What a reader gets wrong: a run is repeated by itself only before any server call; the verb is a pure function of the Vfs until then.

import type { CompletionItem } from "@studio/commands/commandSpec";
import { editReady } from "@studio/k7-wasm/edit";
import { injectRoots, useVirtualDisk } from "@studio/song/injectedRoots";
import type { CommandInfo } from "@studio/web-protocol";
import {
  argClass,
  argHint,
  completionItems,
  loadCommandSpecs,
  runCommand,
} from "../commandSpec.ts";
import {
  LibraryCache,
  type Loaded,
  loadProject,
  relativeTo,
} from "./bundle.ts";
import { CompileNeedsFiles, type Compiler, wasmCompiler } from "./compiler.ts";
import { installGlobals } from "./globals.ts";
import {
  type HostParts,
  makeBrowserHost,
  type Output,
  type RunFlags,
  resetDrafts,
  syncDrafts,
} from "./host.ts";
import { HostError } from "./members/jobs.ts";
import { withLocal } from "./members/local.ts";
import {
  type VideoDocAnswer,
  type VideoErrorsAnswer,
  videoDocOf,
  videoErrorsOf,
} from "./pictureViews.ts";
import type { ChangeOp, ChangesAnswer, Remote } from "./remote.ts";
import { setVfs } from "./shims/current.ts";
import { Unsupported, unsupportedLine } from "./unsupported.ts";
import { NeedsFiles } from "./vfs.ts";

/** How many times a run or a completion is repeated for files it found missing: a compile discovers one library folder per round. */
const MAX_FETCH_ROUNDS = 32;

export interface RunOptions {
  /** Merge into the journal step on top, as the bar's repeated edits do. */
  journal?: "join" | "skip";
  want?: string[];
}

/** What a run answers: the lines, the revision either side, what it wrote. */
export interface RunResult {
  output: Output;
  before: string;
  after: string;
  changed: { path: string; kind: string }[];
  graph?: unknown;
  /** How many times the verb ran: 1, plus one per fetch of missing files and one for a moved project. */
  runs: number;
  committed: boolean;
  /** `output.refused` as a field: a structured clone drops an array's own properties. */
  refused: boolean;
  /** The host member a refusal is for, when the verb needs one the Worker lacks. */
  unsupported?: string;
}

export interface RuntimeOptions {
  remote: Remote;
  compiler?: Compiler | (() => Promise<Compiler>);
  libraries?: LibraryCache;
}

export interface Runtime {
  open(at: string): Promise<void>;
  run(line: string, o?: RunOptions): Promise<RunResult>;
  complete(
    line: string,
    labelled?: boolean,
  ): Promise<(string | CompletionItem)[]>;
  catalogue(): CommandInfo[];
  /** The picture the open project draws; `token` is the server's revision now, and a Vfs on another one is read again first. */
  videoDoc(token?: string): Promise<VideoDocAnswer>;
  videoErrors(token?: string): Promise<VideoErrorsAnswer>;
  /** Files a peer or the server changed, by workspace-relative name: fetched and held; with `rev` the project's new token. */
  refresh(files: string[], rev?: string): Promise<void>;
  close(): void;
  readonly revision: string | undefined;
}

/** The roots a run reads from, as the bundle's request spelled them (virtual): handed to song, and the Worker's `$STUDIO_*` pointed at them. */
async function withRoots<T>(l: Loaded, fn: () => Promise<T>): Promise<T> {
  const roots = (
    l.bundle.source.request as
      | {
          roots?: {
            models?: string;
            scan?: string;
            meta?: string;
            libraries?: { name: string; root: string }[];
          };
        }
      | undefined
  )?.roots;
  if (!roots) return fn();
  const env = process.env;
  const was = {
    models: env.STUDIO_MODELS,
    home: env.STUDIO_HOME,
    libs: env.STUDIO_LIBRARIES_HOME,
  };
  const studio = roots.scan?.slice(0, roots.scan.lastIndexOf("/"));
  if (roots.models) env.STUDIO_MODELS = roots.models;
  if (studio) env.STUDIO_HOME = studio;
  const libraries = roots.libraries ?? [];
  const account = libraries.find((x) => x.name === "account");
  injectRoots({
    librariesHome: "/l",
    accountHome: account?.root ?? `${studio ?? ""}/library`,
    libraries,
    meta: roots.meta ?? null,
    installed: l.bundle.installed ?? {},
  });
  try {
    return await fn();
  } finally {
    injectRoots(null);
    for (const [k, v] of [
      ["STUDIO_MODELS", was.models],
      ["STUDIO_HOME", was.home],
      ["STUDIO_LIBRARIES_HOME", was.libs],
    ] as const) {
      if (v === undefined) delete env[k];
      else env[k] = v;
    }
  }
}

/** The member a missing-route error names: the Worker's own `Unsupported`, or the host's `HostError` coded `unsupported`. */
function unsupportedOf(e: unknown): string | undefined {
  if (e instanceof Unsupported) return e.member;
  if (e instanceof HostError && e.code === "unsupported")
    return /^(\w+):/.exec(e.message)?.[1] ?? "a host member";
  return undefined;
}

export const NO_PROJECT = "no active project — open a project first";

export function createRuntime(o: RuntimeOptions): Runtime {
  installGlobals();
  useVirtualDisk();
  const libraries = o.libraries ?? new LibraryCache();
  let compilerP: Promise<Compiler> | undefined;
  const compiler = (): Promise<Compiler> => {
    compilerP ??= Promise.resolve(
      typeof o.compiler === "function"
        ? o.compiler()
        : (o.compiler ?? wasmCompiler()),
    );
    return compilerP;
  };
  let at = "";
  // The member the last run's refusal names: runs take turns, so one slot is enough.
  let unsupported: string | undefined;
  let loaded: Loaded | undefined;
  let parts: HostParts | undefined;
  let rev: string | undefined;
  // Every token this runtime has held since it opened: a view asked at one of them is older than what it holds, never newer.
  const held = new Set<string>();
  const hold = (now: string | undefined): void => {
    rev = now;
    if (now) held.add(now);
  };
  // A close aborts the polls of a run still waiting; the server's heartbeat then stops the jobs.
  let closing = new AbortController();

  async function open(project: string): Promise<void> {
    // The verbs' edits are synchronous calls into the module: it is compiled before any verb runs.
    if (!(await editReady()))
      throw new Error("WebAssembly is not available in this runtime");
    const c = await compiler();
    if (parts) c.close(parts.handle);
    at = project;
    loaded = await loadProject(o.remote, project, libraries);
    setVfs(loaded.vfs);
    held.clear();
    hold(loaded.rev);
    parts = {
      vfs: loaded.vfs,
      compiler: c,
      handle: c.open(loaded.songBundle),
      project: loaded.project,
      at: project,
      workspaceRoot: loaded.workspaceRoot,
      remote: o.remote,
      signal: closing.signal,
      sent: new Map(),
    };
  }

  /** Fetches what a read found unknown into the Vfs and the compiler; false when nothing was asked. */
  async function fetchNeeded(paths: string[]): Promise<boolean> {
    if (!loaded || !parts || paths.length === 0) return false;
    const { vfs, workspaceRoot } = loaded;
    const drafts: { path: string; text?: string }[] = [];
    for (const path of [...new Set(paths)]) {
      const rel = relativeTo(workspaceRoot, path);
      // Outside the workspace (the machine's own files) the browser can fetch nothing: it reads as absent.
      if (rel === null) {
        vfs.markAbsent(path);
        // The compiler is told too, or it asks for the same path every round.
        drafts.push({ path });
        continue;
      }
      const got =
        (await o.remote.file(rel)) ??
        (o.remote.list ? await listed(rel, path) : null);
      if (got === null) {
        vfs.markAbsent(path);
        drafts.push({ path });
      } else if (got === "listed") continue;
      else if ("text" in got) {
        vfs.load(path, {
          text: got.text,
          len: new TextEncoder().encode(got.text).length,
        });
        drafts.push({ path, text: got.text });
      } else vfs.load(path, { len: got.len, binary: true });
    }
    if (drafts.length) parts.compiler.update(parts.handle, drafts);
    return true;
  }

  async function listed(rel: string, path: string): Promise<"listed" | null> {
    const entries = await o.remote.list?.(rel);
    if (!entries || !loaded) return null;
    loaded.vfs.loadList(path, entries);
    return "listed";
  }

  function opsOf(l: Loaded): ChangeOp[] {
    const spell = (p: string): string => relativeTo(l.workspaceRoot, p) ?? p;
    const ops: ChangeOp[] = [];
    for (const op of l.vfs.staged()) {
      if (op.kind === "text")
        ops.push({
          kind: "text",
          file: spell(op.file),
          text: op.text,
          was: op.was,
        });
      else if (op.kind === "remove")
        ops.push({ kind: "remove", file: spell(op.file), was: op.was });
      else if (op.kind === "adopt")
        ops.push({
          kind: "adopt",
          job: op.job,
          out: op.out,
          file: spell(op.file),
          ...(op.replace ? { replace: true } : {}),
        });
      else
        ops.push({
          kind: op.kind,
          from: spell(op.from),
          file: spell(op.file),
          ...(op.replace ? { replace: true } : {}),
        });
    }
    const written = new Set(ops.map((x) => x.file));
    // A file read outside the project is checked too, so a stale read makes the set a 409.
    for (const [path, was] of l.vfs.reads) {
      // A library's files are published and still; only the chain being edited is checked inside one.
      if (path.startsWith("/l/") && !path.startsWith(`${l.project}/`)) continue;
      const rel = relativeTo(l.workspaceRoot, path);
      if (rel !== null && !written.has(rel))
        ops.push({ kind: "read", file: rel, was });
    }
    return ops;
  }

  /** The server moved under the run: the named files are fetched (or the whole project, when only its token moved). */
  async function catchUp(answer: ChangesAnswer): Promise<void> {
    if (!loaded || !parts) return;
    const { stale = [], project, rev: now } = answer.body;
    if (project || stale.length === 0) {
      await open(at);
      return;
    }
    const abs = stale.map((f) =>
      f.startsWith("/") ? f : `${loaded?.workspaceRoot}/${f}`,
    );
    for (const p of abs) loaded.vfs.forget(p);
    await fetchNeeded(abs);
    if (now) hold(now);
  }

  async function run(line: string, ro: RunOptions = {}): Promise<RunResult> {
    unsupported = undefined;
    const r = await runLine(line, ro);
    return {
      ...r,
      refused: r.output.refused === true,
      ...(unsupported ? { unsupported } : {}),
    };
  }

  async function runLine(
    line: string,
    ro: RunOptions,
  ): Promise<Omit<RunResult, "refused" | "unsupported">> {
    const output: Output = [];
    if (!loaded || !parts) {
      const no: Output = [NO_PROJECT];
      no.refused = true;
      return {
        output: no,
        before: "",
        after: "",
        changed: [],
        runs: 0,
        committed: false,
      };
    }
    setVfs(loaded.vfs);
    let runs = 0;
    let moved = false;
    for (let round = 0; ; round++) {
      const l = loaded as Loaded;
      const p = parts as HostParts;
      output.length = 0;
      output.refused = undefined;
      const flags: RunFlags = { reload: false, hostCalled: false };
      runs++;
      l.vfs.takeNeeded();
      let threw = false;
      try {
        await withRoots(l, () =>
          runCommand(
            line,
            withLocal(
              makeBrowserHost(p, output, flags),
              flags,
              l.bundle.preferences,
            ),
            "cli",
          ),
        );
      } catch (e) {
        threw = true;
        const gap = unsupportedOf(e);
        if (gap) {
          unsupported = gap;
          output.length = 0;
          output.push(unsupportedLine(line, gap));
          output.refused = true;
        } else if (
          !(e instanceof NeedsFiles) &&
          !(e instanceof CompileNeedsFiles)
        )
          output.push(`error: ${(e as Error).message}`);
        if (e instanceof CompileNeedsFiles && (await fetchNeeded(e.files))) {
          l.vfs.rollback();
          resetDrafts(p);
          if (round < MAX_FETCH_ROUNDS) continue;
          output.push(
            `error: still missing after ${round} fetches (the compile): ${e.files.join(", ")}`,
          );
        }
      }
      // A verb that caught the missing member's throw and went on is refused whole, never half done.
      if (flags.unsupported && !unsupported) {
        unsupported = flags.unsupported;
        output.length = 0;
        output.push(unsupportedLine(line, flags.unsupported));
        output.refused = true;
      }
      // A verb that swallowed a NeedsFiles throw is still wrong: what it was told was "unknown", not "absent".
      const need = l.vfs.takeNeeded();
      if (need.length && round < MAX_FETCH_ROUNDS) {
        l.vfs.rollback();
        resetDrafts(p);
        await fetchNeeded(need);
        continue;
      }
      if (need.length)
        output.push(
          `error: still missing after ${round} fetches (a read): ${need.join(", ")}`,
        );
      const before = rev ?? "";
      if (threw || output.refused || need.length) {
        l.vfs.rollback();
        resetDrafts(p);
        return {
          output,
          before,
          after: before,
          changed: [],
          runs,
          committed: false,
        };
      }
      const ops = opsOf(l);
      if (ops.every((x) => x.kind === "read")) {
        l.vfs.settle();
        return {
          output,
          before,
          after: before,
          changed: [],
          runs,
          committed: false,
        };
      }
      const answer = await o.remote.changes({
        at,
        base: before,
        label: line,
        ...(ro.journal ? { journal: ro.journal } : {}),
        ops,
        ...(flags.reload ? { reload: true } : {}),
        ...(ro.want ? { want: ro.want } : {}),
      });
      if (answer.status === 200) {
        l.vfs.settle();
        syncDrafts(p);
        p.sent.clear();
        hold(answer.body.rev ?? rev);
        // A file the compiler cannot be told of (a render adopted, a copy of audio) changes what the project means: read it again.
        if (ops.some((x) => ["adopt", "copy", "move"].includes(x.kind)))
          await open(at);
        return {
          output,
          before: answer.body.revBefore ?? before,
          after: answer.body.rev ?? before,
          changed: answer.body.changed ?? [],
          graph: answer.body.graph,
          runs,
          committed: true,
        };
      }
      l.vfs.rollback();
      resetDrafts(p);
      const wrong = answer.body.open !== undefined;
      if (answer.status === 409 && !wrong) {
        if (flags.hostCalled || moved) {
          await catchUp(answer);
          output.push("the project moved — run it again");
          return {
            output,
            before,
            after: before,
            changed: [],
            runs,
            committed: false,
          };
        }
        moved = true;
        await catchUp(answer);
        continue;
      }
      throw new Error(
        answer.body.error ?? `/changes answered ${answer.status}`,
      );
    }
  }

  async function complete(
    line: string,
    labelled = false,
  ): Promise<(string | CompletionItem)[]> {
    if (!loaded || !parts) return [];
    setVfs(loaded.vfs);
    for (let round = 0; ; round++) {
      const l = loaded as Loaded;
      l.vfs.takeNeeded();
      let items: CompletionItem[] = [];
      let ask: string[] = [];
      try {
        items = completionItems(
          line,
          makeBrowserHost(parts as HostParts, [], {
            reload: false,
            hostCalled: false,
          }),
          "cli",
        );
      } catch (e) {
        if (e instanceof CompileNeedsFiles) ask = e.files;
      }
      ask = [...ask, ...l.vfs.takeNeeded()];
      if (ask.length && round < MAX_FETCH_ROUNDS) {
        await fetchNeeded(ask);
        continue;
      }
      return items.map((c) => (labelled && c.label ? c : c.line));
    }
  }

  /** A read-only view of the project over the Vfs: fetched again for what it found unknown, on a Vfs current with `token`. */
  async function view<T>(read: (l: Loaded) => T, token?: string): Promise<T> {
    if (!loaded || !parts) throw new Error(NO_PROJECT);
    // A token the runtime has passed through is the page reading late: what the runtime holds is as new, so no open is paid.
    if (token !== undefined && !held.has(token)) await open(at);
    for (let round = 0; ; round++) {
      const l = loaded as Loaded;
      setVfs(l.vfs);
      l.vfs.takeNeeded();
      let value: T | undefined;
      let failure: unknown;
      try {
        value = await withRoots(l, async () => read(l));
      } catch (e) {
        failure = e;
      }
      const ask = l.vfs.takeNeeded();
      if (ask.length && round < MAX_FETCH_ROUNDS) {
        await fetchNeeded(ask);
        continue;
      }
      if (failure !== undefined) throw failure;
      return value as T;
    }
  }

  function catalogue(): CommandInfo[] {
    return loadCommandSpecs()
      .filter((c) => c.surfaces.includes("cli"))
      .map((c) => ({
        command: c.command,
        help: c.help,
        readable: c.readable,
        menu: c.menu,
        confirm: c.confirm,
        args:
          c.args?.map((a) => ({
            name: a.name,
            kind: argClass(a),
            provider: a.complete,
            hint: argHint(a),
            optional: a.optional,
          })) ?? [],
      }));
  }

  async function refresh(files: string[], now?: string): Promise<void> {
    if (!loaded || !parts) return;
    setVfs(loaded.vfs);
    const abs = files.map((f) => `${loaded?.workspaceRoot}/${f}`);
    for (const p of abs) loaded.vfs.forget(p);
    await fetchNeeded(abs);
    if (now) hold(now);
  }

  // One Vfs and one models root per runtime: two overlapping calls would roll each other back, so they take turns.
  let turn: Promise<unknown> = Promise.resolve();
  const serial = <A extends unknown[], T>(fn: (...a: A) => Promise<T>) => {
    return (...a: A): Promise<T> => {
      const next = turn.then(() => fn(...a));
      turn = next.catch(() => undefined);
      return next;
    };
  };

  return {
    open: serial(open),
    run: serial(run),
    complete: serial(complete),
    catalogue,
    videoDoc: serial((token?: string) =>
      view((l) => videoDocOf(l.project, l.workspaceRoot), token),
    ),
    videoErrors: serial((token?: string) =>
      view((l) => videoErrorsOf(l.project), token),
    ),
    refresh: serial(refresh),
    close() {
      closing.abort();
      closing = new AbortController();
      if (parts) parts.compiler.close(parts.handle);
      parts = undefined;
      loaded = undefined;
    },
    get revision() {
      return rev;
    },
  };
}
