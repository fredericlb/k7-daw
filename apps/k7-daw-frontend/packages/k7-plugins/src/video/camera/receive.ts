// receive.ts — the viewer's half of a lens handshake, for the account's cameras (./remoteCamera.ts):
// want, answer the offer, trickle ICE, resolve once.
// What reads wrong: the opener's `send` writes the wire shape; this file never serialises a frame itself.

import type { Lens } from "@studio/ui-eyes/camera";
import { m } from "../../paraglide/messages.js";

/** What this half reads from either socket; anything else is ignored. */
interface ViewerIn {
  type: string;
  sdp?: { type: string; sdp?: string };
  problem?: string | null;
  text?: string;
  candidate?: RTCIceCandidateInit;
}

/** What this half writes; the opener's `send` turns it into its own socket's frame. */
type ViewerOut =
  | { type: "want"; label: string }
  | { type: "ice"; candidate: RTCIceCandidateInit }
  | { type: "answer"; sdp: RTCSessionDescriptionInit };

/** One opened link: its socket and peer, and how frames are read and written on it. */
interface ViewerLink {
  socket: WebSocket;
  peer: RTCPeerConnection;
  read: (data: unknown) => ViewerIn | null;
  send: (f: ViewerOut) => void;
  /** Extra fields for the lens a track resolves, read once the peer exists. */
  extra?: Partial<Lens>;
}

/** A sentence, said in the locale of the moment it is needed. */
type Say = () => string;

export interface ReceiveOptions<T> {
  label: string;
  waitMs: number;
  messages: { noAnswer: Say; closed: Say; noRelay: Say };
  /** What the link needs before its socket opens; a throw or a rejection is the lane's sentence. */
  prepare: () => T | Promise<T>;
  open: (prepared: T) => ViewerLink;
  /** Called whenever the peer is let go: on close, and when its connection fails or closes. */
  onPeerGone?: (peer: RTCPeerConnection) => void;
}

/** Ask for the lens a label names. Resolves once, with a picture or a sentence. */
export function receiveLens<T>(o: ReceiveOptions<T>): Promise<Lens> {
  return new Promise<Lens>((resolve) => {
    let settled = false;
    let link: ViewerLink | undefined;
    // Both ends of the link, closed together — on a failure here, and by the driver when the lane goes.
    const close = () => {
      if (link) {
        o.onPeerGone?.(link.peer);
        link.peer.close();
      }
      const ws = link?.socket;
      if (ws && ws.readyState <= WebSocket.OPEN) ws.close();
    };
    const done = (l: Lens) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      // No picture: nothing will ever be drawn from this link, so it is not kept open.
      if (!l.stream) close();
      resolve(l.stream ? { ...l, close } : l);
    };
    const fail = (e: unknown) =>
      done({ stream: null, problem: (e as Error).message });
    const timer = window.setTimeout(
      () => done({ stream: null, problem: o.messages.noAnswer() }),
      o.waitMs,
    );

    const start = (prepared: T) => {
      if (settled) return;
      try {
        link = o.open(prepared);
      } catch (e) {
        return fail(e);
      }
      wire(link);
    };

    const wire = ({ socket, peer, read, send, extra }: ViewerLink) => {
      /** The sentence the host sent with its offer, kept for whichever resolves first. */
      let problem: string | null = null;
      peer.onicecandidate = (e) => {
        if (e.candidate) send({ type: "ice", candidate: e.candidate.toJSON() });
      };
      peer.ontrack = (e) =>
        done({
          stream: e.streams[0] ?? new MediaStream([e.track]),
          problem,
          ...extra,
        });

      const handle = async (f: ViewerIn) => {
        if (f.type === "problem")
          return done({ stream: null, problem: f.text ?? null });
        if (f.type === "offer") {
          problem = f.problem ?? null;
          await peer.setRemoteDescription(f.sdp as RTCSessionDescriptionInit);
          const sdp = await peer.createAnswer();
          await peer.setLocalDescription(sdp);
          send({ type: "answer", sdp: peer.localDescription ?? sdp });
          return;
        }
        if (f.type === "ice")
          await peer.addIceCandidate(f.candidate).catch(() => undefined);
      };

      socket.onopen = () => send({ type: "want", label: o.label });
      socket.onmessage = (e) => {
        const f = read(e.data);
        // A negotiation that throws leaves a lane with no picture, which is a sentence and not a crash.
        if (f)
          void handle(f).catch((err: Error) =>
            done({
              stream: null,
              problem: m["lens.failed"]({ error: err.message }),
            }),
          );
      };
      // A socket that closes before a track arrived is a lane that will never draw; say so.
      socket.onclose = () =>
        done({ stream: null, problem: o.messages.closed() });
      socket.onerror = () =>
        done({ stream: null, problem: o.messages.noRelay() });
      // The socket stays open for the connection's life: ICE trickles after the first frame.
      peer.onconnectionstatechange = () => {
        if (
          peer.connectionState === "closed" ||
          peer.connectionState === "failed"
        ) {
          o.onPeerGone?.(peer);
          socket.close();
        }
      };
    };

    let prepared: T | Promise<T>;
    try {
      prepared = o.prepare();
    } catch (e) {
      return fail(e);
    }
    if (prepared instanceof Promise) prepared.then(start, fail);
    else start(prepared);
  });
}
