// api.ts — the typed client. Three destinations now:
//   • the hosted backend (same-origin at the ROOT, cookie): the local user's session at
//     /auth/* — k7 is the service with no prefix — and this app's own preferences.
//   • the engine-side file service (cross-origin, header-auth): all file/workspace ops.
//   • the front's routes (this origin, cookie): the ONE engine registry, and the songs a playlist names.
// The file service base + the identity/workspace/engine-token headers are set by
// configureFileApi once an engine is chosen and the workspace is resolved (see ProjectGate).

import { engineAccess } from "@studio/engine-browser/access";
import type { EnginePrefs } from "@studio/engine-browser/registry";
import type { ParamMeta } from "@studio/engine-protocol";
import { DSP_FILE, JSDSP_FILE } from "@studio/engine-protocol/build";
// The reader `@studio/engine-browser` wants: it owns the routes, this app owns the errors.
import { readJson as json } from "@studio/http-client/call";
import { upload } from "@studio/http-client/upload";
import { WIDGETS_SCRIPT } from "@studio/k7-wasm/consts";
import type { AiRequest, AiResult } from "@studio/k7-wasm/panel";
import type { NodePresets, ReferenceTrack } from "@studio/k7-wasm/types";
import { readOrganizeStream } from "@studio/panel-rules/organize-stream";
import { cfg, fget, fileHeaders, fsend } from "@studio/project-shell/fileApi";
import { projectApi } from "@studio/project-shell/project";
import type { ModuleStore } from "@studio/ui-module-builder/store";
import { MAX_UPLOAD_BYTES, type ModelDownloadJob } from "@studio/web-protocol";
import type { PlaylistInfo } from "@studio/web-protocol/playlists";
import type { ProjectResource } from "@studio/web-protocol/resources";
import { serviceUrl } from "@studio/web-protocol/services";
import type { CommandPrefs } from "./commandPrefs";
import type {
  PreviewReq,
  PublicEngine,
  WaveformStatus,
  WebWaveform,
} from "./types";

// The app is mounted at the ORIGIN ROOT, so BASE is the empty string — kept as a derived constant, this app's mount being the one that can still change.
const BASE = import.meta.env.BASE_URL.replace(/\/$/, "");

/** Where the front's routes answer: this origin. It holds the preferences AND the one engine registry. */
const ORIGIN = serviceUrl(globalThis.location?.origin ?? "", "k7");

// ── The hosted backend (same-origin): auth + preferences ────────────────────────

// The registry's five routes are `engines` below — pl8's — and auth is three bare fetches, so there is no `get`/`send` helper here.

/** k7's own preference document — the remembered engine plus the command prefs; widening it is HALF of adding a field, the backend's zod schema deciding the rest. */
export interface K7Prefs extends EnginePrefs {
  commands?: CommandPrefs;
}
/** The registry, the probe and the relay, and the preferences with the remembered engine among them: one origin. */
export const engines = engineAccess<PublicEngine, K7Prefs>(ORIGIN, json, {
  // A workspace the server hosts is opened there, whichever engine plays it — apps/k7-daw-backend/docs/files/hub.md.
  hostedFiles: true,
});

// ── The engine-side file service (cross-origin, header-auth) ─────────────────────

// The transport and the shared routes live in @studio/project-shell; what callers reach through here is the media URL.
export { mediaUrl } from "@studio/project-shell/fileApi";

import { m } from "./paraglide/messages.js";

/** The current session's user, or null if unauthenticated (`/auth/me`). */
export interface AuthUser {
  sub?: string;
  name?: string;
  email?: string | null;
  groups?: string[];
}

/** The sentence for a file over the upload limit — said before sending, and again if the service refuses it. */
const tooBigMessage = (name: string) =>
  m["upload.tooBig"]({ name, mb: String(MAX_UPLOAD_BYTES / (1024 * 1024)) });

/** Refuse a file over the limit BEFORE it is sent: a 4K clip would otherwise climb to 100% and fail with no reason. */
function tooBig(name: string, bytes: Blob | ArrayBuffer): void {
  const size = bytes instanceof Blob ? bytes.size : bytes.byteLength;
  if (size <= MAX_UPLOAD_BYTES) return;
  throw Object.assign(new Error(tooBigMessage(name)), { status: 413 });
}

/** How long a signed-in answer to /auth/me is reused: long enough for a start's several askers, short enough to notice a session gone. */
const ME_FRESH_MS = 60_000;
let me: { at: number; p: Promise<AuthUser | null> } | null = null;

async function askMe(): Promise<AuthUser | null> {
  const res = await fetch("/auth/me");
  if (res.status === 401) return null;
  if (!res.ok) throw new Error(`auth/me ${res.status}`);
  return (await res.json()).user as AuthUser;
}

/** A download's name from `Content-Disposition`: the RFC 5987 `filename*` first, else the plain `filename`. */
export function downloadName(header: string): string | null {
  const star = /filename\*=UTF-8''([^;\s]+)/i.exec(header);
  if (star?.[1]) {
    try {
      return decodeURIComponent(star[1]);
    } catch {
      // A bad escape falls back to the plain name.
    }
  }
  const plain = /filename="?([^";]+)"?/.exec(header);
  return plain?.[1] ?? null;
}

export const api = {
  // --- Auth (the device's own backend, same-origin) ---

  // Shared while in flight and for a minute after a signed-in answer; nobody signed in, or a failure, is asked again.
  getMe: (): Promise<AuthUser | null> => {
    if (me && Date.now() - me.at < ME_FRESH_MS) return me.p;
    const asked = { at: Date.now(), p: askMe() };
    me = asked;
    const forget = () => {
      if (me === asked) me = null;
    };
    asked.p.then((u) => {
      if (!u) forget();
    }, forget);
    return asked.p;
  },

  // --- Engine registry (hosted backend, same-origin) ---

  // What this service remembers about YOU is among them, and is the ACCOUNT's — never `preferences` below, which is the RIG's.

  // --- Workspaces + file ops (engine-side file service, cross-origin) ---

  // The shared half, spread rather than re-listed, so this app and @studio/project-shell cannot drift on a route name.
  ...projectApi,

  // --- The workspace's playlists (`_playlists/`, see apps/k7-daw-wasm/song/docs/playlists.md) ---

  // Only the LISTING is its own route: `read`/`write` above already serve any workspace path, and `playlist new` is a command.
  playlists: () => fget<{ playlists: PlaylistInfo[] }>("/playlists"),

  // --- The workspace's reference tracks (`_references/`, see apps/k7-daw-wasm/song-schema/docs/references.md) ---

  // The LISTING only: a track goes up with `upload` and is played over `mediaUrl`, both of which already take any workspace path.
  references: () => fget<{ references: ReferenceTrack[] }>("/references"),

  // The active project's imported audio and video clips, played over `mediaUrl` (../docs/resources.md).
  resources: () => fget<{ resources: ProjectResource[] }>("/resources"),

  // --- What a project asks of its engine ---

  // Copy the project to the engine's machine again after a copy failed — apps/k7-daw-backend/docs/file-engine.md.
  retryEngineSync: () => fsend<{ ok: true }>("POST", "/engine/sync/retry", {}),
  // Leave an automatically frozen VST live while it is edited — apps/k7-daw-wasm/song/docs/autoFreeze.md.
  thawAutoFreeze: (module: string, on = true) =>
    fsend<{ ok: true }>("POST", "/auto-freeze/thaw", { module, on }),
  // --- A node's model, fetched onto this machine (the right-click's "Download model") ---

  /** Start fetching the model a NODE names from its module's `model_url`; `job: null` is "already here". */
  downloadModel: (node: string) =>
    fsend<{ job: ModelDownloadJob | null }>("POST", "/models/download", {
      node,
    }),
  /** …and how far it is — account-level on the service, polled by the settings too. */
  modelDownload: (id: string) =>
    fget<ModelDownloadJob>(`/account/models/${encodeURIComponent(id)}`),
  // Raw bytes with progress (octet-stream, not fsend); a refusal rides through with `.status` so a 409 skip can be reported.
  uploadWithProgress: async (
    dir: string,
    name: string,
    bytes: Blob | ArrayBuffer,
    onProgress?: (loaded: number, total: number) => void,
  ): Promise<{ path: string }> => {
    tooBig(name, bytes);
    const q = `dir=${encodeURIComponent(dir)}&name=${encodeURIComponent(name)}`;
    const headers = Object.fromEntries(
      Object.entries(
        fileHeaders({ "Content-Type": "application/octet-stream" }),
      ).map(([k, v]) => [k, String(v)]),
    );
    const size = bytes instanceof Blob ? bytes.size : bytes.byteLength;
    let res: Response;
    try {
      res = await upload(cfg().base + `/upload?${q}`, {
        headers,
        body: bytes,
        onProgress: (f) => onProgress?.(f * size, size),
      });
    } catch (e) {
      const why = e instanceof DOMException ? "cancelled" : "network error";
      throw new Error(`upload ${name} — ${why}`);
    }
    if (!res.ok) {
      const msg =
        res.status === 413
          ? tooBigMessage(name)
          : ((await res.json().catch(() => ({}))).error ??
            `upload ${name} failed`);
      const err = new Error(msg) as Error & { status?: number };
      err.status = res.status;
      throw err;
    }
    // A 2xx that is not JSON (a proxy's page) must still settle, or the drop's progress stays up for good.
    return (await res.json().catch(() => {
      throw new Error(`upload ${name} — the answer was not readable`);
    })) as { path: string };
  },
  // `whole` spans the source file rather than the placement, and `sensitivity` re-detects the SOURCE's transients, writing the threshold into the sidecar.
  waveform: (
    clip: string,
    buckets = 1000,
    whole = false,
    sensitivity?: number,
  ) =>
    fget<WebWaveform>(
      `/waveform?clip=${encodeURIComponent(clip)}&buckets=${buckets}${whole ? "&whole=1" : ""}${
        sensitivity === undefined ? "" : `&sensitivity=${sensitivity}`
      }`,
    ),
  // Audition one sample: the file service resolves it to a file and window, fires it at the engine, and answers the window it sent.
  preview: (req: PreviewReq) =>
    fsend<{ ok: true; start: number; length: number }>("POST", "/preview", req),
  // Render the mixdown and hand back blob + filename. "mp3" needs ffmpeg on the engine host; "wav" always works.
  exportMix: async (
    format: "wav" | "mp3",
    // One timeline alone, as it is auditioned; absent, Main.
    timeline?: string,
  ): Promise<{ blob: Blob; filename: string }> => {
    const tl = timeline ? `&timeline=${encodeURIComponent(timeline)}` : "";
    const res = await fetch(cfg().base + `/export?format=${format}${tl}`, {
      headers: fileHeaders(),
    });
    if (!res.ok)
      throw new Error(
        (await res.json().catch(() => ({}))).error ?? "export failed",
      );
    const cd = res.headers.get("Content-Disposition") || "";
    return {
      blob: await res.blob(),
      filename: downloadName(cd) ?? `mixdown.${format}`,
    };
  },
  // The tracks zip's address: a link the browser streams, so the workspace rides in the query, as `mediaUrl`'s does.
  tracksUrl: (token: string, timeline?: string) => {
    const q = new URLSearchParams({ workspace: cfg().workspace, dl: token });
    if (timeline) q.set("timeline", timeline);
    return `${cfg().base}/export/tracks?${q.toString()}`;
  },
  // The tracks zip as a blob, for a host that saves it into the person's folder rather than streaming it to Downloads.
  exportTracks: async (
    timeline?: string,
  ): Promise<{ blob: Blob; filename: string }> => {
    const q = new URLSearchParams({ workspace: cfg().workspace });
    if (timeline) q.set("timeline", timeline);
    const res = await fetch(`${cfg().base}/export/tracks?${q}`, {
      headers: fileHeaders(),
    });
    if (!res.ok)
      throw new Error(
        (await res.json().catch(() => ({}))).error ?? "export failed",
      );
    return {
      blob: await res.blob(),
      filename:
        downloadName(res.headers.get("Content-Disposition") || "") ??
        "tracks.zip",
    };
  },
  prewarmWaveforms: () =>
    fsend<WaveformStatus>("POST", "/waveforms/prewarm", {}, { fence: false }),
  waveformStatus: () => fget<WaveformStatus>("/waveforms/status"),
  // Stop the batch: it decodes audio in a second engine process, which is not what the machine should be doing while the transport plays.
  cancelWaveformPrewarm: () =>
    fsend<WaveformStatus>("POST", "/waveforms/cancel", {}, { fence: false }),

  /** Ask the MACHINE what an `@engine/…` plugin has, by loading one on purpose — SECONDS, and `null` is 'cannot load', not an empty list. */
  scanPluginParams: async (
    ref: string,
  ): Promise<{ names: string[]; meta: Record<string, ParamMeta> } | null> => {
    const { found, params } = await fget<{
      found?: boolean;
      params?: { name: string; min: number; max: number; def?: number }[];
    }>(`/plugin-params?ref=${encodeURIComponent(ref)}`);
    if (found === false) return null;
    const list = params ?? [];
    return {
      names: list.map((p) => p.name),
      meta: Object.fromEntries(
        list.map((p) => [p.name, { min: p.min, max: p.max, def: p.def }]),
      ),
    };
  },

  /** …and that same plugin's own WINDOW, on an ORPHAN instance: nothing touched in it is saved, and it opens on the ENGINE's machine. */
  showPluginNativeUI: (ref: string) =>
    fget<{ opened: string }>(
      `/plugin-native-ui?ref=${encodeURIComponent(ref)}`,
    ),

  /** The bases this module may be set to — its own and its ancestors'; the SERVER derives the folder from the module path it is given. */
  moduleBases: (path: string) =>
    fget<{ bases: string[] }>(`/module-base?path=${encodeURIComponent(path)}`),

  snapshotModuleBase: (path: string, name: string, ref: string) =>
    fsend<{ path: string }>("POST", "/module-base/snapshot", {
      path,
      name,
      ref,
    }),

  // A node's presets, files under a presets tree; applying and saving are `node preset` verbs — apps/k7-daw-wasm/song/docs/userPresets.md.
  nodePresets: (node: string) =>
    fget<NodePresets>(`/presets?node=${encodeURIComponent(node)}`),
};

/** The API root, for the callers beside this file that name a route of its own (the engines' chains and slots). */
export const API_ROOT = `${ORIGIN}/api`;

/** What an empty DSP source is pre-filled with, per language — a stub that LOADS, since an empty file fails later and less obviously. */
function dspStub(ref: string, file: string): string {
  const head = `// ${ref} — the module's DSP.`;
  // Keyed by FILE and not by a default, so a language nobody wrote a stub for gets a comment rather than the wrong language's boilerplate.
  if (file === DSP_FILE)
    return `${head} Params and graph, as data: see the DSP authoring page.\nparams({ Level: { min: 0, max: 1, default: 1 } })\ngraph([output(amul(input(0), "Level"))])\n`;
  if (file === JSDSP_FILE)
    return `${head} Export the graph the engine will host.\nexport default {};\n`;
  return `${head}\n`;
}

/** What an absent `widgets.js` opens on: one screen, drawn, so the contract is on the page. */
function widgetsStub(ref: string): string {
  return `// ${ref} — what the panel's lcdscreen widgets draw, keyed by widget id.
// lcd: { ctx, width, height, params, input, output, state, color, now } — see the LCD screen doc.
export default {
  screen(lcd) {
    lcd.ctx.fillStyle = lcd.color.ink;
    lcd.ctx.font = "10px ui-monospace, monospace";
    lcd.ctx.fillText("${ref.split("/").pop()}", 4, 12);
  },
};
`;
}

/** A module's sibling SOURCE by name, or where it would go plus a stub — absent is not an error. */
async function siblingSource(
  path: string,
  file: string,
  stub: (ref: string) => string,
) {
  const parts = path.split("/");
  const ref = parts.slice(0, -1).join("/");
  const at = [...parts.slice(0, -1), file].join("/");
  try {
    const f = await api.read(at);
    return { ref, path: at, content: f.content, exists: true };
  } catch {
    return { ref, path: at, content: stub(ref), exists: false };
  }
}

/** The shared module builder's view of the file service — shapes, not behaviour, except `write`, which may be REDIRECTED onto a project copy (moduleFork.ts). */
export const moduleStore: ModuleStore = {
  // Absent is `null`, not a throw (see ModuleStore.read): a module the project has yet to write is exactly that.
  read: async (path) => {
    try {
      return (await api.read(path)).content;
    } catch (e) {
      if ((e as { status?: number }).status === 404) return null;
      throw e;
    }
  },
  moduleUi: (path) => api.moduleUi(path),
  engines: (path) => api.moduleEngines(path),
  /** The module's DSP source, derived HERE rather than on the server: it is always the plain sibling, and absent is not an error. */
  dspSource: (path, file) =>
    siblingSource(path, file, (ref) => dspStub(ref, file)),
  /** …and the lcdscreen script, the same plain sibling of the same folder. */
  widgetsScript: (path) => siblingSource(path, WIDGETS_SCRIPT, widgetsStub),
  hints: () => api.hints(),
  /** Keep the plugin's current state as a named base — straight through to the route, the only side that knows where `base/` is. */
  snapshotBase: (path, name, ref) => api.snapshotModuleBase(path, name, ref),
  /** …and what it may be set TO: the ancestry is the server's to walk, a thin local module having none of its own. */
  bases: async (path) => (await api.moduleBases(path)).bases,
  write: async (path, content) => {
    const { forked } = await api.write(path, content);
    if (!forked) return {};
    return {
      path: forked.path,
      note:
        `${forked.ref} is shared — saved a project copy as ${forked.module}` +
        (forked.chains.length
          ? `, and repointed ${forked.chains.join(", ")}`
          : ""),
    };
  },
};

/** The account's Claude settings as the browser may see them: the model, and the key only as a hint. */
export interface AiSettingsView {
  model: string | null;
  defaultModel: string;
  keyHint: string | null;
}

/** The module builder's "AI organize", on THIS origin's front — the one holding the account's key (apps/k7-daw-backend/docs/files.md). */
export const aiOrganize = {
  settings: async (): Promise<AiSettingsView> =>
    json(await fetch(`${BASE}/api/ai/settings`), "ai/settings"),
  /** A PATCH: an absent field is kept, null clears it — the key is never sent back to be kept. */
  saveSettings: async (patch: {
    apiKey?: string | null;
    model?: string | null;
  }): Promise<AiSettingsView> =>
    json(
      await fetch(`${BASE}/api/ai/settings`, {
        method: "PUT",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(patch),
      }),
      "ai/settings",
    ),
  /** Whether the button should exist at all; any failure reads as "no", since the button is optional. */
  enabled: async (): Promise<boolean> => {
    const res = await fetch(`${BASE}/api/ai/organize`).catch(() => null);
    if (!res?.ok) return false;
    return ((await res.json()) as { enabled?: boolean }).enabled === true;
  },
  /** One run on THIS origin's front, which holds the account's key and runs the whole conversation: NDJSON of progress, then the panel or the error. */
  run: async (
    req: AiRequest,
    onStatus: (text: string) => void,
  ): Promise<AiResult> => {
    const res = await fetch(`${BASE}/api/ai/organize`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(req),
    });
    if (!res.ok || !res.body) return json<never>(res, "ai/organize");
    return readOrganizeStream(res.body, onStatus);
  },
};

/** The graph's thaw, fire-and-forget: a control on an automatically frozen VST asks, and the reload is the answer. */
export function thawAutoFreeze(module: string): void {
  api.thawAutoFreeze(module).catch(() => {});
}
