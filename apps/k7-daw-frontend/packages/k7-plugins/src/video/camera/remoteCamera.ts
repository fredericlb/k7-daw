// remoteCamera.ts — a lens on one of the ACCOUNT's remote cameras: a device in camera mode, reached
// through k7's backend, which only carries the signalling — the picture comes peer to peer. A layer
// names one as `remote:<device name>`, and it is opened, drawn and recorded like a local lens.
// Every remote lens this page holds is sampled, and their jitter buffers evened out (./latency.ts).
// See apps/k7-daw-frontend/packages/web-protocol/docs/cameras.md.

import type { LensOpener } from "@studio/ui-eyes/camera";
import {
  CAMERAS_ROUTE,
  CAMERAS_WS_PATH,
  type CameraFrame,
  type CameraTicket,
  type RemoteCamera,
  readCameraFrame,
} from "@studio/web-protocol/cameras";
import { m } from "../../paraglide/messages.js";
import {
  bufferTargets,
  type LatencySample,
  sampleFromStats,
  totalMs,
} from "./latency";
import { receiveLens } from "./receive";

/** Long enough for a camera on a phone to answer, short enough that a lane says something. */
const WAIT_MS = 15_000;
/** How often the open remote lenses are measured and evened out. */
const SAMPLE_MS = 2000;

/** The account's cameras that are on; empty when the backend has none or cannot be asked. */
export async function listRemoteCameras(): Promise<RemoteCamera[]> {
  try {
    const res = await fetch(CAMERAS_ROUTE, { credentials: "same-origin" });
    return res.ok ? ((await res.json()) as RemoteCamera[]) : [];
  } catch {
    return [];
  }
}

/** A ticket for the signalling socket, and the ICE servers the server hands out. */
export async function cameraTicket(): Promise<CameraTicket> {
  const res = await fetch(`${CAMERAS_ROUTE}/ticket`, {
    method: "POST",
    credentials: "same-origin",
  });
  if (!res.ok) throw new Error(`camera ticket: HTTP ${res.status}`);
  return (await res.json()) as CameraTicket;
}

/** The socket's address on this origin, which is the server the account's cameras are on. */
export function camerasSocketUrl(params: Record<string, string>): string {
  const u = new URL(CAMERAS_WS_PATH, window.location.origin);
  u.protocol = u.protocol === "https:" ? "wss:" : "ws:";
  for (const [k, v] of Object.entries(params)) u.searchParams.set(k, v);
  return u.toString();
}

export const sendCameraFrame = (ws: WebSocket, f: CameraFrame): void => {
  if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(f));
};

/** The remote lenses this page holds, measured together so they can be made equally late. */
const held = new Map<RTCPeerConnection, { sample: LatencySample | null }>();
let sampler = 0;

async function sampleAll(): Promise<void> {
  const peers = [...held.keys()];
  await Promise.all(
    peers.map(async (pc) => {
      const report = await pc.getStats().catch(() => null);
      const entry = held.get(pc);
      if (report && entry)
        entry.sample = sampleFromStats(report.values()) ?? entry.sample;
    }),
  );
  const measured = peers.filter((pc) => held.get(pc)?.sample);
  const targets = bufferTargets(
    measured.map((pc) => held.get(pc)?.sample as LatencySample),
  );
  measured.forEach((pc, i) => {
    for (const r of pc.getReceivers()) {
      // `jitterBufferTarget` is recent (Chrome 114+): without it, lenses are measured and left as they come.
      if (r.track.kind === "video" && "jitterBufferTarget" in r)
        (
          r as RTCRtpReceiver & { jitterBufferTarget: number | null }
        ).jitterBufferTarget = targets[i] || null;
    }
  });
}

function hold(pc: RTCPeerConnection): () => number | null {
  const entry: { sample: LatencySample | null } = { sample: null };
  held.set(pc, entry);
  if (!sampler) sampler = window.setInterval(() => void sampleAll(), SAMPLE_MS);
  // The entry and not the map: a take reads its lens's lateness after closing it.
  return () => (entry.sample ? Math.round(totalMs(entry.sample)) : null);
}

function release(pc: RTCPeerConnection): void {
  held.delete(pc);
  if (!held.size && sampler) {
    clearInterval(sampler);
    sampler = 0;
  }
}

/** Ask the server for the account's camera a label names. Resolves once, with a picture or a sentence. */
export const remoteCameraLens: LensOpener = (label) =>
  receiveLens({
    label,
    waitMs: WAIT_MS,
    messages: {
      noAnswer: m["lens.remoteNoAnswer"],
      closed: m["lens.remoteGone"],
      noRelay: m["lens.remoteNoRelay"],
    },
    prepare: cameraTicket,
    open: ({ ticket, iceServers }) => {
      const socket = new WebSocket(
        camerasSocketUrl({ ticket, role: "viewer" }),
      );
      const peer = new RTCPeerConnection({
        iceServers: iceServers as RTCIceServer[],
      });
      return {
        socket,
        peer,
        read: readCameraFrame,
        // This socket's sdp is a plain `{ type, sdp }`, never a description object.
        send: (f) =>
          sendCameraFrame(
            socket,
            f.type === "answer"
              ? { type: "answer", sdp: { type: f.sdp.type, sdp: f.sdp.sdp } }
              : f,
          ),
        extra: { latencyMs: hold(peer) },
      };
    },
    onPeerGone: release,
  });
