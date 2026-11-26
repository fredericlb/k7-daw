// pageVerbs.ts — the page's VerbsHost: a line run by the backend's `POST /verb`, its completion, the picture and an import, all routes.
// What a reader gets wrong: the page holds no project and builds nothing; every answer is the backend's (docs/verbs-port.md).

import type {
  VerbCompletion,
  VerbRun,
  VerbRunOptions,
  VerbsHost,
  VideoAnswer,
} from "@studio/project-shell/verbsHost";
import type {
  ChangedFile,
  CommandInfo,
  WebVideoProject,
} from "@studio/web-protocol";

/** A run past this is a process of the page, listed beside the server's. */
const LONG_RUN_MS = 1000;

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

/** What `POST /picture` answers: the document and what it found wrong, or the compile's own refusal. */
type PictureAnswer =
  | { doc: WebVideoProject; warnings: string[] }
  | { error: string };

/** What an import answers: the new project's folder and the lines the dialog shows. */
interface ImportResult {
  project: string;
  log: string[];
}

export interface PageVerbsDeps {
  /** A JSON POST to the file service: `fsend`, or a test's stand-in. */
  send: <T>(path: string, body: unknown) => Promise<T>;
  /** …and one that changes no file, so the reads around it stay shared. */
  read: <T>(path: string, body: unknown) => Promise<T>;
  /** The palette's catalogue. */
  catalogue: () => Promise<CommandInfo[]>;
  /** Lists a run in the process list; `end` is told the error, if any. */
  track: (label: string) => { end: (error?: string) => void };
}

export interface PageVerbs extends VerbsHost {
  /** A project file of the workspace the page has open, imported into a new folder by `POST /import`. */
  importPath(rel: string, as?: string): Promise<ImportResult>;
}

/** The lines a run shows: what it said, why it refused, and an escaped throw. */
function outputOf(a: VerbAnswer): string[] {
  return [...a.log, ...a.refuse, ...(a.threw ? [`error: ${a.threw}`] : [])];
}

export function createPageVerbs(d: PageVerbsDeps): PageVerbs {
  const picture = () => d.read<PictureAnswer>("/picture", {});
  return {
    catalogue() {
      return d.catalogue();
    },
    async complete(line) {
      try {
        const r = await d.read<{ items: (string | VerbCompletion)[] }>(
          "/verb/complete",
          { line, labelled: true },
        );
        return r.items;
      } catch {
        // The palette keeps asking: a completion that fails is no items, never an error.
        return [];
      }
    },
    async videoDoc(): Promise<VideoAnswer> {
      const a = await picture();
      return "error" in a ? { error: a.error } : { doc: a.doc };
    },
    async videoErrors() {
      const a = await picture();
      return {
        errors: "error" in a ? [a.error] : a.warnings,
        diagnostics: [] as never[],
      };
    },
    async run(line: string, options: VerbRunOptions): Promise<VerbRun> {
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
      }
    },
    async importPath(rel, as) {
      const shown = d.track(`import ${rel}`);
      try {
        const r = await d.send<ImportResult>("/import", {
          rel,
          ...(as ? { as } : {}),
        });
        shown.end();
        return r;
      } catch (e) {
        shown.end((e as Error).message);
        throw e;
      }
    },
  };
}
