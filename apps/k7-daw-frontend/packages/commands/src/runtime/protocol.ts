// protocol.ts — the messages between a page and the project's Worker (picture views, import), and the one function that serves them.
// What a reader gets wrong: `serve` takes any port with postMessage and "message" events, so a test needs no Worker at all.

import { LibraryCache } from "./bundle.ts";
import type { Importer, ImportResult } from "./importer.ts";
import type { Runtime } from "./runtime.ts";

export type Message =
  | { id: number; type: "init"; base: string; headers?: Record<string, string> }
  | { id: number; type: "open"; at: string }
  | { id: number; type: "videoDoc"; token?: string; at?: string }
  | { id: number; type: "videoErrors"; token?: string; at?: string }
  | { id: number; type: "refresh"; files: string[]; rev?: string; at?: string }
  | { id: number; type: "close"; at?: string }
  | {
      id: number;
      type: "importFile";
      name: string;
      data: Blob | ArrayBuffer;
      as?: string;
    }
  | { id: number; type: "importPath"; rel: string; as?: string };

export type Reply =
  | { id: number; ok: true; value: unknown }
  | { id: number; ok: false; error: string };

/** What a Worker scope, a MessagePort and a node adapter all are. */
export interface PortLike {
  postMessage(message: unknown): void;
  addEventListener(
    type: "message",
    listener: (e: { data: unknown }) => void,
  ): void;
}

type Init = { base: string; headers?: Record<string, string> };

/**
 * Answers each message of `port` from the runtimes `make` builds, one per `at`.
 * A message with no `at` addresses the project opened last; the calls take turns, since the shims hold one Vfs.
 */
export function serve(
  port: PortLike,
  make: (init: Init, libraries: LibraryCache) => Runtime,
  importing?: (init: Init, libraries: LibraryCache) => Importer,
): void {
  const libraries = new LibraryCache();
  const runtimes = new Map<string, Runtime>();
  let init: Init | undefined;
  let current: string | undefined;
  let turn: Promise<unknown> = Promise.resolve();
  const pick = (at?: string): Runtime => {
    const key = at ?? current;
    const rt = key === undefined ? undefined : runtimes.get(key);
    if (!rt) throw new Error("no project is open");
    return rt;
  };
  const answer = async (m: Message): Promise<unknown> => {
    switch (m.type) {
      case "init":
        for (const rt of runtimes.values()) rt.close();
        runtimes.clear();
        current = undefined;
        init = m;
        return null;
      case "open": {
        if (!init) throw new Error("the runtime has not been initialised");
        const rt = runtimes.get(m.at) ?? make(init, libraries);
        runtimes.set(m.at, rt);
        current = m.at;
        return rt.open(m.at);
      }
      case "videoDoc":
        return pick(m.at).videoDoc(m.token);
      case "videoErrors":
        return pick(m.at).videoErrors(m.token);
      case "refresh":
        return pick(m.at).refresh(m.files, m.rev);
      case "importFile":
      case "importPath": {
        if (!init || !importing)
          throw new Error("the runtime has not been initialised");
        const importer = importing(init, libraries);
        return m.type === "importFile"
          ? importer.importFile(m.data, m.name, m.as)
          : importer.importPath(m.rel, m.as);
      }
      case "close": {
        const key = m.at ?? current;
        if (key === undefined) return undefined;
        runtimes.get(key)?.close();
        runtimes.delete(key);
        if (current === key) current = undefined;
        return undefined;
      }
    }
  };
  port.addEventListener("message", (e) => {
    const m = e.data as Message;
    const next = turn.then(async () => {
      try {
        port.postMessage({ id: m.id, ok: true, value: await answer(m) });
      } catch (err) {
        port.postMessage({
          id: m.id,
          ok: false,
          error: (err as Error).message,
        });
      }
    });
    turn = next;
  });
}

type WithoutId<T> = T extends unknown ? Omit<T, "id"> : never;

/** The page's side: each call is a message and a promise settled by the reply with its id. */
export function client(port: PortLike) {
  let next = 0;
  const waiting = new Map<
    number,
    { ok: (v: unknown) => void; fail: (e: Error) => void }
  >();
  port.addEventListener("message", (e) => {
    const r = e.data as Reply;
    const w = waiting.get(r.id);
    if (!w) return;
    waiting.delete(r.id);
    if (r.ok) w.ok(r.value);
    else w.fail(new Error(r.error));
  });
  const send = <T>(m: WithoutId<Message>): Promise<T> =>
    new Promise<T>((ok, fail) => {
      const id = ++next;
      waiting.set(id, { ok: ok as (v: unknown) => void, fail });
      port.postMessage({ id, ...m });
    });
  return {
    /** Settles every call still waiting with `reason`: a Worker that died answers nothing. */
    fail(reason: Error): void {
      for (const w of waiting.values()) w.fail(reason);
      waiting.clear();
    },
    init: (base: string, headers?: Record<string, string>) =>
      send<null>({ type: "init", base, headers }),
    open: (at: string) => send<void>({ type: "open", at }),
    videoDoc: (token?: string, at?: string) =>
      send<Awaited<ReturnType<Runtime["videoDoc"]>>>({
        type: "videoDoc",
        token,
        at,
      }),
    videoErrors: (token?: string, at?: string) =>
      send<Awaited<ReturnType<Runtime["videoErrors"]>>>({
        type: "videoErrors",
        token,
        at,
      }),
    refresh: (files: string[], rev?: string, at?: string) =>
      send<void>({ type: "refresh", files, rev, at }),
    close: (at?: string) => send<void>({ type: "close", at }),
    /** A project file the page holds, imported into a new folder of the workspace the Worker was initialised for. */
    importFile: (data: Blob | ArrayBuffer, name: string, as?: string) =>
      send<ImportResult>({ type: "importFile", name, data, as }),
    /** A project file already in the workspace, imported from where it sits. */
    importPath: (rel: string, as?: string) =>
      send<ImportResult>({ type: "importPath", rel, as }),
  };
}
