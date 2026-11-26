// desktopEngine.ts — this machine's engine, when the page is served by the desktop app's backend, which runs its supervisor:
// the supervisor's latest picture, pushed over /api/local/engine/events, and the few commands a page may send.
// A server or a phone's backend runs none, and the probe says so. See ../../docs/localApp.md.

import { showToast } from "@studio/project-shell/toast";
import { useEffect, useState } from "react";
import { useLocalApp } from "../localApp";
import { m } from "../paraglide/messages.js";

/** What this page draws of the supervisor's snapshot (apps/k7-daw-backend/packages/k7-supervisor/schema/hostProtocol.schema.json). */
export interface DesktopEngineSnapshot {
  status: "stopped" | "running" | "error";
  error?: string;
  userStopped: boolean;
  engineId: string;
  enginePort: number;
  lastHeartbeatOk: boolean;
  pluginCount?: number;
  plugins?: { name: string; format: string; manufacturer: string }[];
  audioDevices?: {
    name: string;
    deviceType: string;
    current: boolean;
    currentInput: boolean;
    /** The open device's channel counts, on its own row; absent or 0 on the others. */
    inputs?: number;
    outputs?: number;
  }[];
  audioPriority?: string[];
  /** Whether a device on this network drives the engine unpaired; absent from an older supervisor. */
  lanOpen?: boolean;
  /** The k7 servers this engine is listed at; one from the environment is not removable. */
  instances?: { url: string; removable?: boolean }[];
  instanceCheck?: { state?: string; url?: string; message?: string } | null;
  instruments?: {
    name: string;
    label: string;
    bytes: number;
    wanted: boolean;
    installed: boolean;
  }[];
  downloads?: {
    name: string;
    received: number;
    total: number;
    phase: "queued" | "downloading" | "unpacking" | "failed";
    error?: string;
  }[];
  /** The frozen chains this machine keeps compiled, and the one it plays with nobody attached (../../docs/slots.md). */
  slots?: {
    id: string;
    name: string;
    chain: string;
    savedAt: string;
    footswitch?: number | null;
  }[];
  enabledSlot?: string | null;
}

/** One instrument as the Downloads section draws it: the catalogue row and where its download stands. */
export interface InstrumentRow {
  name: string;
  label: string;
  bytes: number;
  installed: boolean;
  wanted: boolean;
  phase: "queued" | "downloading" | "unpacking" | "failed" | null;
  received: number;
  error: string | null;
}

/** Every instrument the engine can play, each with its download's phase when one is queued or running. */
export function instrumentRows(
  s: Pick<DesktopEngineSnapshot, "instruments" | "downloads">,
): InstrumentRow[] {
  const downloads = s.downloads ?? [];
  return (s.instruments ?? []).map((i) => {
    const d = downloads.find((x) => x.name === i.name);
    return {
      ...i,
      phase: d?.phase ?? null,
      received: d?.received ?? 0,
      error: d?.error ?? null,
    };
  });
}

/** The row's one action, or none while a download runs: the supervisor cannot cut one mid-file. */
export function instrumentAction(
  i: InstrumentRow,
): { label: "cancel" | "retry" | "remove" | "download"; cmd: string } | null {
  if (i.phase === "downloading" || i.phase === "unpacking") return null;
  if (i.phase === "queued")
    return { label: "cancel", cmd: "instruments.remove" };
  if (i.phase === "failed")
    return { label: "retry", cmd: "instruments.download" };
  if (i.installed) return { label: "remove", cmd: "instruments.remove" };
  return { label: "download", cmd: "instruments.download" };
}

/** "748 MB", "1.3 GB": the size the list pins, which is what the download costs. */
export function sizeOf(bytes: number): string {
  const mb = bytes / 1e6;
  return mb >= 1000
    ? `${(mb / 1000).toFixed(1)} GB`
    : `${Math.max(1, Math.round(mb))} MB`;
}

export interface DesktopEngineState {
  snapshot: DesktopEngineSnapshot | null;
  /** Why the supervisor is away (restarting, failed), or null. */
  down: string | null;
}

/** Whether this page's backend runs the machine's engine; `undefined` while it is being asked. */
export function useOnDesktopEngine(): boolean | undefined {
  const app = useLocalApp();
  return app === undefined ? undefined : !!app?.engine;
}

/** Ask the supervisor; the backend takes only the commands its allow-list names (apps/k7-daw-backend/packages/k7-front/src/routes/local/routes.rs). */
export function engineCommand(cmd: string, arg?: string): void {
  void (async () => {
    // A refusal or a dead backend is said as a toast: a button that does nothing visible is worse than an error.
    try {
      const res = await fetch("/api/local/engine/cmd", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(arg === undefined ? { cmd } : { cmd, arg }),
      });
      if (res.ok) return;
      const said = await res
        .json()
        .then((b: { error?: string }) => b.error)
        .catch(() => undefined);
      toastRefusal(said ?? `${res.status}`);
    } catch (e) {
      toastRefusal((e as Error).message);
    }
  })();
}

function toastRefusal(reason: string): void {
  showToast({
    message: m["engine.commandFailed"]({ reason }),
    variant: "error",
  });
}

/** The engine's state, pushed from the first frame on; null until one arrives, or when there is no engine here. */
export function useDesktopEngine(): DesktopEngineState | null {
  const [state, setState] = useState<DesktopEngineState | null>(null);
  const here = useOnDesktopEngine();
  useEffect(() => {
    if (!here) return;
    // EventSource reconnects on its own when the backend restarts.
    const events = new EventSource("/api/local/engine/events");
    events.onmessage = (e) =>
      setState(JSON.parse(e.data) as DesktopEngineState);
    return () => events.close();
  }, [here]);
  return state;
}

/** U+001F joins the names `audio.priority` takes (apps/k7-daw-backend/packages/k7-supervisor/docs/hostProtocol.md). */
const SEP = "\u001f";

export interface AudioChoice {
  /** The interface the engine opens first: the ranking's head, else the one open now. */
  primary: string | null;
  /** The one it falls back on: the ranking's second name. */
  fallback: string | null;
  /** The devices seen, then a ranked name that is not plugged in. */
  options: { name: string; present: boolean }[];
}

/** What the two audio selects show: the ranking's first two names, over the devices the engine sees. */
export function audioChoice(s: DesktopEngineSnapshot): AudioChoice {
  const devices = s.audioDevices ?? [];
  const priority = s.audioPriority ?? [];
  const options = devices.map((d) => ({ name: d.name, present: true }));
  for (const name of priority.slice(0, 2))
    if (!options.some((o) => o.name === name))
      options.push({ name, present: false });
  return {
    primary: priority[0] ?? devices.find((d) => d.current)?.name ?? null,
    fallback: priority[1] ?? null,
    options,
  };
}

/** The `audio.priority` argument for a primary and its fallback; a fallback equal to the primary is dropped. */
export function priorityArg(primary: string, fallback: string | null): string {
  return (
    fallback && fallback !== primary ? [primary, fallback] : [primary]
  ).join(SEP);
}

/** An ASIO driver the engine is NOT on while it plays through another backend — on Windows, the shared endpoint of the same interface, two channels a side (apps/k7-daw-engine/k7-runtime/docs/audioDevice.md). */
export function idleAsioDriver(s: DesktopEngineSnapshot): string | null {
  const devices = s.audioDevices ?? [];
  const isAsio = (t: string) => /asio/i.test(t);
  const current = devices.find((d) => d.current);
  if (!current || isAsio(current.deviceType)) return null;
  return devices.find((d) => isAsio(d.deviceType))?.name ?? null;
}
