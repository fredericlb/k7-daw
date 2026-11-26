// runtime.ts — the open project's files in the Worker: the bundle loaded into a Vfs, the picture views read over it, kept current.
// What a reader gets wrong: no verb runs here; a verb is the backend's `POST /verb`, and this only reads (docs/runtime.md).

import { editReady } from "@studio/k7-wasm/edit";
import { injectRoots, useVirtualDisk } from "@studio/song/injectedRoots";
import {
  LibraryCache,
  type Loaded,
  loadProject,
  relativeTo,
} from "./bundle.ts";
import { installGlobals } from "./globals.ts";
import {
  type VideoDocAnswer,
  type VideoErrorsAnswer,
  videoDocOf,
  videoErrorsOf,
} from "./pictureViews.ts";
import type { Remote } from "./remote.ts";
import { setVfs } from "./shims/current.ts";

/** How many times a view is read again for files it found missing: a read discovers one library folder per round. */
const MAX_FETCH_ROUNDS = 32;

export interface RuntimeOptions {
  remote: Remote;
  libraries?: LibraryCache;
}

export interface Runtime {
  open(at: string): Promise<void>;
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

export const NO_PROJECT = "no active project — open a project first";

export function createRuntime(o: RuntimeOptions): Runtime {
  installGlobals();
  useVirtualDisk();
  const libraries = o.libraries ?? new LibraryCache();
  let at = "";
  let loaded: Loaded | undefined;
  let rev: string | undefined;
  // Every token this runtime has held since it opened: a view asked at one of them is older than what it holds, never newer.
  const held = new Set<string>();
  const hold = (now: string | undefined): void => {
    rev = now;
    if (now) held.add(now);
  };
  async function open(project: string): Promise<void> {
    // The song read side parses k7x through the module: it is compiled before any view is read.
    if (!(await editReady()))
      throw new Error("WebAssembly is not available in this runtime");
    at = project;
    loaded = await loadProject(o.remote, project, libraries);
    setVfs(loaded.vfs);
    held.clear();
    hold(loaded.rev);
  }

  /** Fetches what a read found unknown into the Vfs; false when nothing was asked. */
  async function fetchNeeded(paths: string[]): Promise<boolean> {
    if (!loaded || paths.length === 0) return false;
    const { vfs, workspaceRoot } = loaded;
    for (const path of [...new Set(paths)]) {
      const rel = relativeTo(workspaceRoot, path);
      // Outside the workspace (the machine's own files) the browser can fetch nothing: it reads as absent.
      if (rel === null) {
        vfs.markAbsent(path);
        continue;
      }
      const got =
        (await o.remote.file(rel)) ??
        (o.remote.list ? await listed(rel, path) : null);
      if (got === null) vfs.markAbsent(path);
      else if (got === "listed") continue;
      else if ("text" in got)
        vfs.load(path, {
          text: got.text,
          len: new TextEncoder().encode(got.text).length,
        });
      else vfs.load(path, { len: got.len, binary: true });
    }
    return true;
  }

  async function listed(rel: string, path: string): Promise<"listed" | null> {
    const entries = await o.remote.list?.(rel);
    if (!entries || !loaded) return null;
    loaded.vfs.loadList(path, entries);
    return "listed";
  }

  /** A read-only view of the project over the Vfs: fetched again for what it found unknown, on a Vfs current with `token`. */
  async function view<T>(read: (l: Loaded) => T, token?: string): Promise<T> {
    if (!loaded) throw new Error(NO_PROJECT);
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

  async function refresh(files: string[], now?: string): Promise<void> {
    if (!loaded) return;
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
    videoDoc: serial((token?: string) =>
      view((l) => videoDocOf(l.project, l.workspaceRoot), token),
    ),
    videoErrors: serial((token?: string) =>
      view((l) => videoErrorsOf(l.project), token),
    ),
    refresh: serial(refresh),
    close() {
      loaded = undefined;
    },
    get revision() {
      return rev;
    },
  };
}
