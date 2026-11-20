// exportHost.ts — where an export goes: the system's folder picker, a file written into the folder it gave, then
// that folder shown or the file handed to the share sheet. One shape over three hosts: the desktop app (the front's
// routes, `window.k7Desktop`), the phone apps (the `k7Android`/`k7Ios` bridge) and a plain browser (a download).
// What a reader gets wrong: a phone's folder is a grant held by the app, so its bytes cross the bridge in chunks.

import { shell } from "./appShell";
import { downloadBlob } from "./downloadBlob";
import { engineCall, onAndroid } from "./settings/androidEngine";

/** A folder the person chose: `id` is the host's own handle for it, `path` its disk path when the front can write there. */
export interface Folder {
  id: string;
  name: string;
  path?: string;
}

/** A file written: its name as saved (a taken name is numbered) and the host's handle for it. */
export interface Saved {
  folder: Folder;
  name: string;
  where: string;
}

export interface ExportHost {
  kind: "desktop" | "phone" | "browser";
  /** The host has a folder picker; false in a browser, where an export is a download. */
  canPick: boolean;
  canReveal: boolean;
  canShare: boolean;
  /** The system folder picker, opened where the last export went; null when cancelled. */
  pickFolder(): Promise<Folder | null>;
  save(folder: Folder, name: string, blob: Blob): Promise<Saved>;
  reveal(saved: Saved): Promise<void>;
  share(saved: Saved): Promise<void>;
}

/** Bytes per bridge call: the phone's own download path moves 512 KiB at a time too. */
export const CHUNK = 512 * 1024;

const BROWSER_FOLDER: Folder = { id: "", name: "Downloads" };

const none = (): Promise<never> =>
  Promise.reject(new Error("this device cannot do that"));

const browser: ExportHost = {
  kind: "browser",
  canPick: false,
  canReveal: false,
  canShare: false,
  pickFolder: async () => BROWSER_FOLDER,
  save: async (folder, name, blob) => {
    downloadBlob(blob, name);
    return { folder, name, where: name };
  },
  reveal: none,
  share: none,
};

async function post(path: string, body?: unknown): Promise<Response> {
  const res = await fetch(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
  if (!res.ok)
    throw new Error(
      ((await res.json().catch(() => ({}))) as { error?: string }).error ??
        `${res.status}`,
    );
  return res;
}

export const desktop: ExportHost = {
  kind: "desktop",
  canPick: true,
  canReveal: true,
  canShare: false,
  pickFolder: async () => {
    const got = (await (await post("/api/local/export/folder")).json()) as {
      path?: string;
      name?: string;
    };
    return got.path
      ? { id: got.path, path: got.path, name: got.name ?? "" }
      : null;
  },
  save: async (folder, name, blob) => {
    const q = new URLSearchParams({ folder: folder.id, name });
    const res = await fetch(`/api/local/export/file?${q}`, {
      method: "PUT",
      body: blob,
    });
    if (!res.ok)
      throw new Error(
        ((await res.json().catch(() => ({}))) as { error?: string }).error ??
          `${res.status}`,
      );
    const done = (await res.json()) as { path: string; name: string };
    return { folder, name: done.name, where: done.path };
  },
  reveal: async (saved) => {
    await post("/api/local/export/reveal", { path: saved.where });
  },
  share: none,
};

/** Base64 of a byte range, in pieces the call stack takes. */
async function base64Of(blob: Blob): Promise<string> {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let text = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    text += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(text);
}

export const phone: ExportHost = {
  kind: "phone",
  canPick: true,
  // iOS has no file manager to name; Android's picker already shows where the file went.
  canReveal: false,
  canShare: true,
  pickFolder: async () => {
    const got = await engineCall<{
      id?: string;
      name?: string;
      path?: string;
    }>("pickExportFolder");
    return got.id
      ? {
          id: got.id,
          name: got.name ?? "",
          ...(got.path ? { path: got.path } : {}),
        }
      : null;
  },
  save: async (folder, name, blob) => {
    const { token } = await engineCall<{ token: string }>("exportBegin", {
      folder: folder.id,
      name,
    });
    try {
      for (let at = 0; at < blob.size; at += CHUNK)
        await engineCall("exportChunk", {
          token,
          data: await base64Of(blob.slice(at, at + CHUNK)),
        });
    } catch (e) {
      await engineCall("exportEnd", { token, cancel: "1" }).catch(() => {});
      throw e;
    }
    const done = await engineCall<{ name: string; where: string }>(
      "exportEnd",
      { token },
    );
    return { folder, name: done.name, where: done.where };
  },
  reveal: none,
  share: async (saved) => {
    await engineCall("shareExport", { where: saved.where, name: saved.name });
  },
};

/** The host this page runs in. */
export function exportHost(): ExportHost {
  if (onAndroid) return phone;
  return shell.desktop ? desktop : browser;
}
