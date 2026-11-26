// verbs.ts — the verbs for the specs: a line through `POST /verb`, a completion through `POST /verb/complete`, the picture from the Worker runtime's node build.
// What a reader gets wrong: the runner is built here on first use (esbuild, a third of a second), not by the bed, and it is asked to OPEN the project before every line, so a spec's own writes are never stale.

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
// @ts-expect-error the build script is plain JavaScript
import { bundleRuntime } from "../../apps/k7-daw-frontend/packages/commands/scripts/bundleRuntime.mjs";

/** What a run answers, the route's shape: the lines, the token either side, what it wrote, and the graph when asked. */
export interface VerbAnswer {
  output: string[];
  refused?: boolean;
  rev?: string;
  revBefore?: string;
  changed?: { path: string; kind: string }[];
  graph?: unknown;
  /** Whether the line's writes landed. */
  committed?: boolean;
}

interface Runner {
  open(at: string): Promise<void>;
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
  catalogue(): { command: string }[];
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

/** The node runner, which answers the picture views. */
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

/** One JSON POST to the service, as the page's `fsend` sends it. */
async function post<T>(
  svc: VerbService,
  workspace: string,
  path: string,
  body: unknown,
  extra: Record<string, string> = {},
): Promise<T> {
  const r = await fetch(`${svc.base}${path}`, {
    method: "POST",
    headers: {
      ...svc.headers(workspace),
      ...extra,
      "content-type": "application/json",
    },
    body: JSON.stringify(body),
  });
  const answer = (await r.json().catch(() => ({}))) as T & { error?: string };
  if (!r.ok) throw new Error(answer.error ?? `${path} answered ${r.status}`);
  return answer;
}

/** What `POST /verb` answers. */
interface RouteAnswer {
  log: string[];
  refuse: string[];
  refused: boolean;
  threw?: string;
  landed?: boolean;
  changed?: { path: string; kind: string }[];
  rev?: string;
  revBefore?: string;
  graph?: unknown;
}

/** Runs one line; `at` is the project as the workspace spells it, empty for the open one. */
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
  const r = await post<RouteAnswer>(
    svc,
    workspace,
    "/verb",
    {
      line,
      want: o.want ?? ["graph"],
      ...(at ? { at } : {}),
      ...(o.journal ? { journal: o.journal } : {}),
    },
    o.headers,
  );
  return {
    output: [...r.log, ...r.refuse, ...(r.threw ? [`error: ${r.threw}`] : [])],
    ...(r.refused ? { refused: true } : {}),
    rev: r.rev,
    revBefore: r.revBefore,
    changed: r.changed ?? [],
    committed: r.landed === true,
    ...(r.graph !== undefined ? { graph: r.graph } : {}),
  };
}

/** The catalogue, as the page builds it from commands.yaml. */
export async function catalogueOf(): Promise<{ command: string }[]> {
  return (await nodeRuntime()).catalogue();
}

/** A completion on the open project, as the command bar asks it. */
export async function completeOf(
  svc: VerbService,
  workspace: string,
  line: string,
): Promise<unknown[]> {
  return (
    await post<{ items: unknown[] }>(svc, workspace, "/verb/complete", { line })
  ).items;
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
