// pageVerbs.ts — the page's VerbsHost: a line run by the backend's `POST /verb`, its completion `POST /verb/complete`, the picture views in the Worker.
// What a reader gets wrong: the Worker holds a project only for its picture views; after a verb wrote, it is handed the files that changed.

import type { ImportResult } from "@studio/commands/runtime/importer";
import { fget } from "@studio/project-shell/fileApi";
import type {
  VerbCompletion,
  VerbRun,
  VerbRunOptions,
  VerbsHost,
} from "@studio/project-shell/verbsHost";
import type { ChangedFile, CommandInfo } from "@studio/web-protocol";
import { VERBS_RESET, type Verbs } from "./verbsWorker.ts";

/** A run past this is a process of the page, listed beside the server's. */
const LONG_RUN_MS = 1000;
/** A foreign change settles this long before the project is read again. */
const REOPEN_MS = 500;

/** What `POST /verb` answers: the crate's answer, what landed and the tokens either side (apps/k7-daw-backend/docs/file-changes.md). */
interface VerbAnswer {
  log: string[];
  refuse: string[];
  refused: boolean;
  threw?: string;
  landed?: boolean;
  changed?: ChangedFile[];
  rev?: string;
  revBefore?: string;
  graph?: unknown;
}

export interface PageVerbsDeps {
  verbs: Verbs;
  /** A JSON POST to the file service: `fsend`, or a test's stand-in. */
  send: <T>(path: string, body: unknown) => Promise<T>;
  /** The palette's catalogue. */
  catalogue: () => Promise<CommandInfo[]>;
  /** The file API's session as the Worker needs it: its key changes when the workspace or the ticket does. */
  session: () => { key: string };
  /** The open project, as `/changes` names it; empty when none is open. */
  activeProject: () => Promise<string>;
  /** Lists a run in the process list; `end` is told the error, if any. */
  track: (label: string) => { end: (error?: string) => void };
}

export interface PageVerbs extends VerbsHost {
  /** The project changed under the page: its runtime is read again after the changes settle. */
  foreignChange(token: string | null): void;
  /** The project the page has open, so a verb need not ask the service which. */
  setProject(path: string): void;
  /** The page left its project. */
  forgetProject(): void;
  /** A project file of the workspace the page has open, imported into a new folder by the Worker. */
  importPath(rel: string, as?: string): Promise<ImportResult>;
}

/** The lines a run shows: what it said, why it refused, and an escaped throw. */
function outputOf(a: VerbAnswer): string[] {
  return [...a.log, ...a.refuse, ...(a.threw ? [`error: ${a.threw}`] : [])];
}

export function createPageVerbs(d: PageVerbsDeps): PageVerbs {
  let key = "";
  const opened = new Set<string>();
  const opening = new Map<string, Promise<void>>();
  // The token the last verb answered: a change to anything else is not ours.
  let known: string | null = null;
  let project: string | undefined;
  let running = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function fresh(): void {
    const now = d.session().key;
    if (now === key) return;
    if (key !== "") d.verbs.reset();
    key = now;
    opened.clear();
    opening.clear();
  }

  /** The project the Worker reads the picture of, opened once for the calls that ask at the same time. */
  async function at(): Promise<string> {
    fresh();
    const target = project ?? (await d.activeProject());
    if (opened.has(target)) return target;
    let p = opening.get(target);
    if (!p) {
      const session = key;
      p = d.verbs.open(target).then(() => {
        if (key === session) opened.add(target);
      });
      opening.set(target, p);
      const forget = () => {
        if (opening.get(target) === p) opening.delete(target);
      };
      p.then(forget, forget);
    }
    await p;
    return target;
  }

  /** A read the session's change failed in flight, asked once more of the new Worker: nobody else asks it again. */
  async function read<T>(ask: () => Promise<T>): Promise<T> {
    try {
      return await ask();
    } catch (e) {
      if ((e as Error).message !== VERBS_RESET) throw e;
      return ask();
    }
  }

  async function reopen(): Promise<void> {
    if (running > 0) return;
    for (const p of [...opened]) {
      try {
        await d.verbs.open(p);
      } catch {
        opened.delete(p);
      }
    }
  }

  /** The files a verb wrote, handed to the Worker's runtimes so a picture read next is the project as it is now. */
  async function refreshWorker(a: VerbAnswer): Promise<void> {
    const files = (a.changed ?? []).map((c) => c.path);
    if (!files.length) return;
    for (const p of [...opened]) {
      try {
        await d.verbs.refresh(files, a.rev, p);
      } catch {
        opened.delete(p);
      }
    }
  }

  return {
    catalogue() {
      return d.catalogue();
    },
    async complete(line) {
      try {
        const r = await d.send<{ items: (string | VerbCompletion)[] }>(
          "/verb/complete",
          { line, labelled: true },
        );
        return r.items;
      } catch {
        // The palette keeps asking: a completion that fails is no items, never an error.
        return [];
      }
    },
    videoDoc: () =>
      read(async () => {
        const target = await at();
        const { rev } = await fget<{ rev: string }>("/revision");
        return d.verbs.videoDoc(rev, target);
      }),
    videoErrors: () =>
      read(async () => {
        const target = await at();
        const { rev } = await fget<{ rev: string }>("/revision");
        return d.verbs.videoErrors(rev, target);
      }),
    async run(line: string, options: VerbRunOptions): Promise<VerbRun> {
      running++;
      let shown: { end: (error?: string) => void } | undefined;
      const long = setTimeout(() => {
        shown = d.track(line);
      }, LONG_RUN_MS);
      try {
        const r = await d.send<VerbAnswer>("/verb", {
          line,
          want: options.want,
          ...(options.at ? { at: options.at } : {}),
          ...(options.journal ? { journal: options.journal } : {}),
        });
        if (r.landed) known = r.rev ?? known;
        if (r.landed) await refreshWorker(r);
        shown?.end();
        return {
          output: outputOf(r),
          refused: r.refused,
          ...(r.graph !== undefined ? { graph: r.graph } : {}),
          ...(r.rev !== undefined ? { rev: r.rev } : {}),
          ...(r.revBefore !== undefined ? { revBefore: r.revBefore } : {}),
          ...(r.landed ? { changed: r.changed ?? [] } : {}),
        };
      } catch (e) {
        shown?.end((e as Error).message);
        throw e;
      } finally {
        clearTimeout(long);
        running--;
      }
    },
    async importPath(rel, as) {
      fresh();
      const shown = d.track(`import ${rel}`);
      try {
        const r = await d.verbs.importPath(rel, as);
        shown.end();
        return r;
      } catch (e) {
        shown.end((e as Error).message);
        throw e;
      }
    },
    foreignChange(token) {
      if (token === null || token === known || opened.size === 0) return;
      clearTimeout(timer);
      timer = setTimeout(() => void reopen(), REOPEN_MS);
    },
    setProject(path) {
      const left = project;
      project = path;
      // The project the page left keeps no runtime: its memory, and a restart reopening it.
      if (left && left !== path) {
        opened.delete(left);
        opening.delete(left);
        void d.verbs.close(left).catch(() => {});
      }
    },
    forgetProject() {
      clearTimeout(timer);
      project = undefined;
      known = null;
      opened.clear();
      opening.clear();
    },
  };
}

/** The open project as the workspace reports it, "" when there is none. */
export async function fetchActiveProject(): Promise<string> {
  const w = await fget<{ activeProject?: string | null }>("/workspace");
  return w.activeProject ?? "";
}
