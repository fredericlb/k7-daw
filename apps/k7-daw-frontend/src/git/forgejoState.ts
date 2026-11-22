// forgejoState.ts — the two screens of a Forgejo server as data: adding one (address, check, registration) and signing in.
// What a reader gets wrong: nothing here holds a token or a secret; the client id is public, the grant stays in the front.

import type { ForgePoll, ForgeProbe, ForgeStart } from "./forgeState";

export type AddServer =
  | { phase: "address" }
  | { phase: "checking" }
  | { phase: "register"; probe: ForgeProbe }
  | { phase: "adding"; probe: ForgeProbe }
  | { phase: "failed"; message: string; probe?: ForgeProbe };

export type AddEvent =
  | { type: "check" }
  | { type: "probed"; probe: ForgeProbe }
  | { type: "add" }
  | { type: "failed"; message: string }
  | { type: "reset" };

export const startAdding: AddServer = { phase: "address" };

/** The probe a failed add can go back to, so Add can be pressed again. */
export const probeOf = (s: AddServer): ForgeProbe | undefined =>
  "probe" in s ? s.probe : undefined;

/** The screen between events; the probe survives a failed add, so the person does not check again. */
export function addAfter(s: AddServer, e: AddEvent): AddServer {
  switch (e.type) {
    case "reset":
      return startAdding;
    case "check":
      return { phase: "checking" };
    case "probed":
      return { phase: "register", probe: e.probe };
    case "add": {
      const probe = probeOf(s);
      return probe ? { phase: "adding", probe } : s;
    }
    case "failed":
      return { phase: "failed", message: e.message, probe: probeOf(s) };
  }
}

/** The ssh port field: empty is the default, anything else a whole port or "bad". */
export function sshPort(typed: string): number | undefined | "bad" {
  const t = typed.trim();
  if (!t) return undefined;
  const n = Number(t);
  return Number.isInteger(n) && n > 0 && n < 65536 ? n : "bad";
}

/** Add is ready with a client id typed and no bad port. */
export const canAdd = (clientId: string, port: string): boolean =>
  clientId.trim() !== "" && sshPort(port) !== "bad";

export type SignIn =
  | { phase: "starting" }
  | { phase: "waiting"; url: string }
  | { phase: "expired" }
  | { phase: "denied" }
  | { phase: "failed"; message: string }
  | { phase: "done"; login: string; keyNote?: string };

export type SignInEvent =
  | { type: "started"; start: ForgeStart }
  | { type: "polled"; answer: ForgePoll }
  | { type: "failed"; message: string }
  | { type: "again" };

export const startingSignIn: SignIn = { phase: "starting" };

/** How often the page asks whether the browser tab finished, in milliseconds. */
export const POLL_MS = 1500;

/** The screen between events; an answer that arrives out of its phase changes nothing. */
export function signInAfter(s: SignIn, e: SignInEvent): SignIn {
  switch (e.type) {
    case "again":
      return startingSignIn;
    case "failed":
      return { phase: "failed", message: e.message };
    case "started":
      return { phase: "waiting", url: e.start.authorizeUrl };
    case "polled": {
      if (s.phase !== "waiting") return s;
      const a = e.answer;
      if (a.state === "expired") return { phase: "expired" };
      if (a.state === "denied") return { phase: "denied" };
      if (a.state === "error")
        return { phase: "failed", message: a.error ?? "" };
      if (a.state === "ok")
        return { phase: "done", login: a.login ?? "", keyNote: a.keyNote };
      return s;
    }
  }
}
