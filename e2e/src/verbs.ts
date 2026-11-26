// verbs.ts — the verbs for the specs, as the page sends them: a line through `POST /verb`, a completion, the picture and an import.
// What a reader gets wrong: nothing here holds a project; every answer is the backend's, so a spec's own writes are never stale.

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

/** What an import answers: the new project's folder and the lines the dialog shows. */
export interface ImportAnswer {
  project: string;
  log: string[];
}

/** The service as the specs reach it: its URL and the headers a spec's calls carry. */
export interface VerbService {
  base: string;
  headers(workspace: string): Record<string, string>;
}

/** One POST to the service, its answer thrown as its sentence when it is not a 2xx. */
async function post<T>(
  svc: VerbService,
  workspace: string,
  path: string,
  body: BodyInit,
  headers: Record<string, string>,
): Promise<T> {
  const r = await fetch(`${svc.base}${path}`, {
    method: "POST",
    headers: { ...svc.headers(workspace), ...headers },
    body,
  });
  const answer = (await r.json().catch(() => ({}))) as T & { error?: string };
  if (!r.ok) throw new Error(answer.error ?? `${path} answered ${r.status}`);
  return answer;
}

/** One JSON POST, as the page's `fsend` sends it. */
const postJson = <T>(
  svc: VerbService,
  workspace: string,
  path: string,
  body: unknown,
  extra: Record<string, string> = {},
): Promise<T> =>
  post<T>(svc, workspace, path, JSON.stringify(body), {
    ...extra,
    "content-type": "application/json",
  });

/** A project file in the workspace, imported into a new folder by `POST /import`. */
export function importPathVia(
  svc: VerbService,
  workspace: string,
  rel: string,
  as?: string,
): Promise<ImportAnswer> {
  return postJson(svc, workspace, "/import", { rel, ...(as ? { as } : {}) });
}

/** …and a file the caller holds, its bytes posted as the home page's drop posts them. */
export function importFileVia(
  svc: VerbService,
  workspace: string,
  data: Blob,
  name: string,
  as?: string,
): Promise<ImportAnswer> {
  return post(svc, workspace, "/import", data, {
    "content-type": "application/octet-stream",
    "x-k7-file": encodeURIComponent(name),
    ...(as ? { "x-k7-as": encodeURIComponent(as) } : {}),
  });
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
  const r = await postJson<RouteAnswer>(
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

/** A completion on the open project, as the command bar asks it. */
export async function completeOf(
  svc: VerbService,
  workspace: string,
  line: string,
): Promise<unknown[]> {
  return (
    await postJson<{ items: unknown[] }>(svc, workspace, "/verb/complete", {
      line,
    })
  ).items;
}

/** The open project's picture and its warnings, as `POST /picture` answers the page; a refusal is thrown as its sentence. */
export async function pictureOf(
  svc: VerbService,
  workspace: string,
): Promise<{
  doc: unknown;
  errors: { errors: string[]; diagnostics: [] };
}> {
  const r = await postJson<
    { doc: unknown; warnings: string[] } | { error: string }
  >(svc, workspace, "/picture", {});
  if ("error" in r) throw new Error(r.error);
  return { doc: r.doc, errors: { errors: r.warnings, diagnostics: [] } };
}
