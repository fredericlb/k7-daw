// identity.ts — the one identity of this device: the name and email that sign its commits and head the user menu.
// It lives in the backend's identity file (Settings, the commit dialog); this module is its page-side mirror.

import { useEffect, useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import { findAccountFiles } from "../settings/accountFiles";
import { type GitTransport, git, type Identity } from "./gitApi";
import { accountTransport } from "./transports";

let known: Identity | null = null;
const listeners = new Set<() => void>();

/** Tells every reader the identity changed, whoever saved it. */
export function setKnownIdentity(id: Identity | null): void {
  known = id;
  for (const l of listeners) l();
}

/** The menu's heading: the name, else the email, else "Local". */
export const headingOf = (id: Identity | null): string =>
  id?.name.trim() || id?.email.trim() || m["home.localUser"]();

/** Saves through the git route and updates every reader; throws what the route refused. */
export async function saveIdentity(
  t: GitTransport,
  id: Identity,
): Promise<Identity> {
  const next = { name: id.name.trim(), email: id.email.trim() };
  await git.setIdentity(t, next);
  // The route's answer carries the sources of the fields still empty; the typed ones are all this page knows.
  const read = await git.identity(t).catch(() => null);
  setKnownIdentity(read ?? next);
  return next;
}

/** Asks the account's file service for the stored identity; null when nothing answers. */
async function readIdentity(): Promise<Identity | null> {
  const files = await findAccountFiles(false);
  if (!files) return null;
  const id = await git.identity(accountTransport(files));
  setKnownIdentity(id);
  return id;
}

/** The identity, null until the first read answers. */
export function useIdentity(): Identity | null {
  useEffect(() => {
    let live = true;
    void readIdentity().catch(() => {
      if (live && known === null) setKnownIdentity({ name: "", email: "" });
    });
    return () => {
      live = false;
    };
  }, []);
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => known,
  );
}
