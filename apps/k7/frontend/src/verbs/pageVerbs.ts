// pageVerbs.ts — the page's VerbsHost: the Worker handle, one runtime per project, the session it was started for, a long run in the process list.
// What a reader gets wrong: a change to the file API's session (workspace, ticket) ends the Worker, and a foreign revision reopens the project in the background.

import type { ImportResult } from "@studio/commands/runtime/importer";
import { fget } from "@studio/project-shell/fileApi";
import type {
  VerbRun,
  VerbRunOptions,
  VerbsHost,
} from "@studio/project-shell/verbsHost";
import { VERBS_RESET, type Verbs } from "./verbsWorker.ts";

/** A run past this is a process of the page, listed beside the server's. */
const LONG_RUN_MS = 1000;
/** A foreign change settles this long before the project is read again. */
const REOPEN_MS = 500;

export interface PageVerbsDeps {
  verbs: Verbs;
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

export function createPageVerbs(d: PageVerbsDeps): PageVerbs {
  let key = "";
  const opened = new Set<string>();
  const opening = new Map<string, Promise<void>>();
  // The token the Worker's last commit answered: a change to anything else is not ours.
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

  async function at(options?: { at?: string }): Promise<string> {
    fresh();
    const target = options?.at ?? project ?? (await d.activeProject());
    if (opened.has(target)) return target;
    // The calls a page makes as it opens share one open: each one fetches and compiles the project, a second on a phone.
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

  return {
    catalogue() {
      fresh();
      return d.verbs.catalogue();
    },
    async complete(line) {
      try {
        const project = await at();
        return await d.verbs.complete(line, true, project);
      } catch {
        // The palette keeps asking: a Worker still loading answers after it is ready, never an error.
        return [];
      }
    },
    videoDoc: () =>
      read(async () => {
        const project = await at();
        const { rev } = await fget<{ rev: string }>("/revision");
        return d.verbs.videoDoc(rev, project);
      }),
    videoErrors: () =>
      read(async () => {
        const project = await at();
        const { rev } = await fget<{ rev: string }>("/revision");
        return d.verbs.videoErrors(rev, project);
      }),
    async run(line: string, options: VerbRunOptions): Promise<VerbRun> {
      const project = await at(options);
      running++;
      const long = setTimeout(() => {
        shown = d.track(line);
      }, LONG_RUN_MS);
      let shown: { end: (error?: string) => void } | undefined;
      try {
        const r = await d.verbs.run(
          line,
          {
            want: options.want,
            ...(options.journal ? { journal: options.journal } : {}),
          },
          project,
        );
        if (r.committed) known = r.after;
        shown?.end();
        return {
          output: r.output,
          refused: r.refused,
          ...(r.graph !== undefined ? { graph: r.graph } : {}),
          ...(r.committed
            ? {
                rev: r.after,
                revBefore: r.before,
                changed: r.changed as never,
              }
            : { rev: r.after, revBefore: r.before }),
          ...(r.unsupported ? { unsupported: r.unsupported } : {}),
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
