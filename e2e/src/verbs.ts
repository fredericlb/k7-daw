// verbs.ts — the verbs for the specs: the runtime's node build over the service's routes, one runner per workspace and folder.
// What a reader gets wrong: the runner is built here on first use (esbuild, a third of a second), not by the bed, and it is asked to OPEN the project before every line, so a spec's own writes are never stale.

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime } from "../../packages/commands/scripts/bundleRuntime.mjs";

/** What a run answers, the route's shape: the lines, the token either side, what it wrote, and the graph when asked. */
export interface VerbAnswer {
  output: string[];
  refused?: boolean;
  unsupported?: string;
  rev?: string;
  revBefore?: string;
  changed?: { path: string; kind: string }[];
  graph?: unknown;
  /** How many times the verb ran, and whether a set landed: for a spec that wants to know. */
  runs?: number;
  committed?: boolean;
}

interface Runner {
  open(at: string): Promise<void>;
  run(
    line: string,
    o?: { want?: string[]; journal?: "join" | "skip" },
  ): Promise<{
    output: string[];
    refused: boolean;
    unsupported?: string;
    before: string;
    after: string;
    changed: { path: string; kind: string }[];
    graph?: unknown;
    runs: number;
    committed: boolean;
  }>;
  complete(line: string, labelled?: boolean): Promise<unknown[]>;
  catalogue(): { command: string }[];
  videoDoc(token?: string): Promise<{ doc: unknown } | { error: string }>;
  videoErrors(token?: string): Promise<{ errors: string[]; diagnostics: [] }>;
  close(): void;
}

type Make = (o: {
  base: string;
  headers: () => Record<string, string>;
}) => Runner;

/** What an import answers: the new project's folder and the lines the dialog shows. */
export interface ImportAnswer {
  project: string;
  log: string[];
}

interface Importer {
  importFile(data: Blob, name: string, as?: string): Promise<ImportAnswer>;
  importPath(rel: string, as?: string): Promise<ImportAnswer>;
}

type MakeImporter = (o: {
  base: string;
  headers: () => Record<string, string>;
}) => Importer;

interface NodeRuntime {
  createNodeRunner: Make;
  createNodeImporter: MakeImporter;
}

let built: Promise<NodeRuntime> | undefined;

/** The node build, made once per process into a folder of its own. */
function nodeRuntime(): Promise<NodeRuntime> {
  built ??= (async () => {
    const out = mkdtempSync(join(tmpdir(), "k7-runtime-node-"));
    await bundleRuntime({ target: "node", outdir: out });
    return (await import(
      pathToFileURL(join(out, "node.mjs")).href
    )) as NodeRuntime;
  })();
  return built;
}

/** The node runner. */
async function runnerMaker(): Promise<Make> {
  return (await nodeRuntime()).createNodeRunner;
}

/** A project file in the workspace, imported by the runtime's node build: the Worker's import, over the service's routes. */
export async function importPathVia(
  svc: VerbService,
  workspace: string,
  rel: string,
  as?: string,
): Promise<ImportAnswer> {
  const { createNodeImporter } = await nodeRuntime();
  return createNodeImporter({
    base: svc.base,
    headers: () => svc.headers(workspace),
  }).importPath(rel, as);
}

/** …and a file the caller holds, as the home page's drop hands it over. */
export async function importFileVia(
  svc: VerbService,
  workspace: string,
  data: Blob,
  name: string,
  as?: string,
): Promise<ImportAnswer> {
  const { createNodeImporter } = await nodeRuntime();
  return createNodeImporter({
    base: svc.base,
    headers: () => svc.headers(workspace),
  }).importFile(data, name, as);
}

/** The service as the runner reaches it: its URL and the headers a spec's calls carry. */
export interface VerbService {
  base: string;
  headers(workspace: string): Record<string, string>;
}

const runners = new Map<string, Runner>();

async function runnerFor(
  svc: VerbService,
  workspace: string,
  at: string,
  extra: Record<string, string> = {},
): Promise<Runner> {
  const key = `${svc.base}|${workspace}|${at}|${JSON.stringify(extra)}`;
  let r = runners.get(key);
  if (!r) {
    const make = await runnerMaker();
    r = make({
      base: svc.base,
      headers: () => ({ ...svc.headers(workspace), ...extra }),
    });
    runners.set(key, r);
  }
  // Opened again for every line: a spec writes around the runner, and a stale read costs more than the open.
  await r.open(at);
  return r;
}

/** Runs one line; `at` is the project as the workspace spells it, empty for no project. */
export async function runVerb(
  svc: VerbService,
  workspace: string,
  at: string,
  line: string,
  o: {
    want?: string[];
    journal?: "join" | "skip";
    /** Headers the spec's own calls carry, such as the engine it bound. */
    headers?: Record<string, string>;
  } = {},
): Promise<VerbAnswer> {
  const r = await (await runnerFor(svc, workspace, at, o.headers)).run(line, {
    want: o.want ?? ["graph"],
    ...(o.journal ? { journal: o.journal } : {}),
  });
  return {
    output: [...r.output],
    ...(r.refused ? { refused: true } : {}),
    ...(r.unsupported ? { unsupported: r.unsupported } : {}),
    rev: r.after,
    revBefore: r.before,
    changed: r.changed,
    runs: r.runs,
    committed: r.committed,
    ...(r.graph !== undefined ? { graph: r.graph } : {}),
  };
}

/** The catalogue and a completion, which need a runner and a project only for the second. */
export async function catalogueOf(
  svc: VerbService,
  workspace: string,
): Promise<{ command: string }[]> {
  return (await runnerFor(svc, workspace, "")).catalogue();
}

export async function completeOf(
  svc: VerbService,
  workspace: string,
  at: string,
  line: string,
): Promise<unknown[]> {
  return (await runnerFor(svc, workspace, at)).complete(line, false);
}

/** The open project's picture and its warnings, as the Worker answers them; `token` is the revision the spec just read. */
export async function pictureOf(
  svc: VerbService,
  workspace: string,
  at: string,
  token: string,
): Promise<{
  doc: unknown;
  errors: { errors: string[]; diagnostics: [] };
}> {
  const r = await runnerFor(svc, workspace, at);
  const doc = await r.videoDoc(token);
  if ("error" in doc) throw new Error(doc.error);
  return { doc: doc.doc, errors: await r.videoErrors(token) };
}

/** Closes every runner (a spec file's afterAll). */
export function closeVerbs(): void {
  for (const r of runners.values()) r.close();
  runners.clear();
}
