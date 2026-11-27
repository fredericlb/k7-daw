// cameras.ts — remote cameras: a device in camera mode offers its lens to the SAME account on the
// same server, and a project's camera node names it as `remote:<device name>`. Both halves are here
// because the backend's hub and the browser must agree on the paths, the frames and which camera a
// label names. A name is a label a person reads, never an id. See apps/k7-daw-frontend/packages/web-protocol/docs/cameras.md.

/** The account's cameras: `GET` lists them, `POST <path>/ticket` mints what opens the socket. */
export const CAMERAS_ROUTE = "/api/cameras";

/** The signalling socket, `?ticket=…&role=camera&name=…` or `?ticket=…&role=viewer`. */
export const CAMERAS_WS_PATH = "/api/cameras/ws";

/** What a camera node's label starts with when it names a remote camera rather than a local lens. */
export const REMOTE_CAMERA_PREFIX = "remote:";

import type { CameraFrame } from "./generated";

export type {
  CameraFrame,
  CameraTicket,
  IceCandidate,
  RemoteCamera,
  RTCIceServerLike,
  SessionDescription,
} from "./generated";
export type CameraHello = Extract<CameraFrame, { type: "hello" }>;
export type CameraWant = Extract<CameraFrame, { type: "want" }>;
export type CameraOffer = Extract<CameraFrame, { type: "offer" }>;
export type CameraAnswer = Extract<CameraFrame, { type: "answer" }>;
export type CameraIce = Extract<CameraFrame, { type: "ice" }>;
export type CameraProblem = Extract<CameraFrame, { type: "problem" }>;
export type CameraGone = Extract<CameraFrame, { type: "gone" }>;
export type CameraViewers = Extract<CameraFrame, { type: "viewers" }>;

/** Read a frame, or null for anything that is not one. */
export function readCameraFrame(data: unknown): CameraFrame | null {
  try {
    const f = JSON.parse(String(data)) as CameraFrame;
    return f && typeof f === "object" && typeof f.type === "string" ? f : null;
  } catch {
    return null;
  }
}

/** Does this camera label name a remote camera? */
export const isRemoteLabel = (label: string): boolean =>
  label.startsWith(REMOTE_CAMERA_PREFIX);

/** The device name a remote label carries; `""` asks for whichever camera is on. */
export const remoteName = (label: string): string =>
  label.slice(REMOTE_CAMERA_PREFIX.length).trim();

/** The label a camera node writes for a device. */
export const remoteLabel = (name: string): string =>
  `${REMOTE_CAMERA_PREFIX}${name}`;
