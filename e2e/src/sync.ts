// sync.ts — what the file hub's conformance specs share: their account and their folders read
// back under CONFORMANCE_FILES_ROOT.
// The account key and textHash are spelled here again on purpose: a spec never calls the code it checks.
import { createHash, randomBytes } from "node:crypto";
import { join } from "node:path";
import type { SessionUser } from "@studio/oidc";
import { deviceBed, E2E_USER } from "./session.ts";

/** The file hub's account: its file service and the workspaces the spec makes in it are the suite's alone. */
export const FILES_USER: SessionUser = {
  sub: "conformance-files",
  name: "Conformance Files",
  email: "conformance-files@st0.app",
  groups: [],
};

/** The backend's STUDIO_FILES_ROOT; unset, the on-disk checks skip. */
export const filesRoot = process.env.CONFORMANCE_FILES_ROOT;

/** Why a spec skips its on-disk half. */
export const NO_FILES_ROOT =
  "CONFORMANCE_FILES_ROOT is the backend's STUDIO_FILES_ROOT";

/** An account's folder name: the email's every non-alphanumeric as `_`, and ten hex of its sha256. */
export function accountKey(email: string): string {
  const hash = createHash("sha256").update(email).digest("hex").slice(0, 10);
  return `${email.replace(/[^a-zA-Z0-9]/g, "_")}-${hash}`;
}

/** The account's folder under the files root; only called by a test that skips without it. */
export const accountDir = (user: SessionUser): string =>
  join(
    filesRoot as string,
    // A device has one account, whoever the spec signs in as.
    accountKey((deviceBed() ? E2E_USER : user).email as string),
  );

/** The hash a whole-text op names the text it replaces by: two 32-bit FNV-like lanes folded to 53 bits, as hex. */
export function textHash(text: string): string {
  let h1 = 0xdeadbeef;
  let h2 = 0x41c6ce57;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    h1 = Math.imul(h1 ^ c, 2654435761);
    h2 = Math.imul(h2 ^ c, 1597334677);
  }
  h1 =
    Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^
    Math.imul(h2 ^ (h2 >>> 13), 3266489909);
  h2 =
    Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^
    Math.imul(h1 ^ (h1 >>> 13), 3266489909);
  return (4294967296 * (2097151 & h2) + (h1 >>> 0)).toString(16);
}

/** A workspace name no other run uses. */
export const freshWorkspace = () =>
  `conformance-${randomBytes(5).toString("hex")}`;
