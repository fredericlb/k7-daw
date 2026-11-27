// connectState.ts — what the Connect and Clone form keeps between its events: the address and the host key's trust.
// What a reader gets wrong: the form holds no secret at all, the device's own key signs in and the page sees its public line only.

import { isWebAddress, sshHost } from "@studio/k7-wasm/git";
import type { Remote } from "./gitApi";

export interface FormFields {
  url: string;
  /** A server key waiting for the person's yes, or one that was confirmed. */
  trust: { fingerprint: string; confirmed: boolean } | null;
  /** The server shows another key than the one trusted: nothing goes on. */
  changed: { expected: string; got: string } | null;
}

export type FormEvent =
  | { type: "edit"; value: string }
  | { type: "unknownHost"; fingerprint: string }
  | { type: "confirm" }
  | { type: "hostKeyChanged"; expected: string; got: string }
  | { type: "hostForgotten" }
  | { type: "refused" }
  | { type: "closed" };

export const emptyForm: FormFields = { url: "", trust: null, changed: null };

export function formAfter(f: FormFields, e: FormEvent): FormFields {
  switch (e.type) {
    case "edit":
      // The address names the server: a change voids what was trusted for the old one.
      return { url: e.value, trust: null, changed: null };
    case "unknownHost":
      return {
        ...f,
        trust: { fingerprint: e.fingerprint, confirmed: false },
        changed: null,
      };
    case "confirm":
      return f.trust ? { ...f, trust: { ...f.trust, confirmed: true } } : f;
    case "hostKeyChanged":
      return {
        ...f,
        trust: null,
        changed: { expected: e.expected, got: e.got },
      };
    case "hostForgotten":
      // With the old key off the list the next call is a first contact again.
      return { ...f, trust: null, changed: null };
    case "closed":
      return emptyForm;
    default:
      return f;
  }
}

export { isWebAddress, sshHost };

/** Whether the form has what a call needs: an address that is not a web one, and a yes to a pending host key. */
export function ready(f: FormFields): boolean {
  return Boolean(
    f.url.trim() &&
      !isWebAddress(f.url) &&
      !f.changed &&
      !(f.trust && !f.trust.confirmed),
  );
}

/** The body of the call; `hostKey` is there only once the person has confirmed that fingerprint. */
export function request(f: FormFields): Remote {
  const base: Remote = { url: f.url.trim() };
  if (f.trust?.confirmed) base.hostKey = f.trust.fingerprint;
  return base;
}
