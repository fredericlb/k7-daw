// injectedRoots.ts — the places a project reads from, handed in by a host that has no machine folders: the browser Worker.
// What a reader gets wrong: unset (the default) nothing here applies; set, every library and `@meta` lookup answers from the table, not the disk layout.

/** One library as the host spelled it: its name, and the folder (the pinned version) it is read from. */
export interface InjectedLibrary {
  name: string;
  root: string;
}

export interface InjectedRoots {
  /** The folder holding one folder per library, for the paths that ask where the libraries live. */
  librariesHome: string;
  /** The account's library, `@account/…`. */
  accountHome: string;
  libraries: InjectedLibrary[];
  meta: string | null;
  /** The published versions each library holds on the machine, with the lowest level that runs each. */
  installed: Record<string, { version: number; requires: number }[]>;
}

let injected: InjectedRoots | null = null;

/** Hands in the roots for the verbs of one run; null puts the machine's own back. */
export function injectRoots(roots: InjectedRoots | null): void {
  injected = roots;
}

export const injectedRootsNow = (): InjectedRoots | null => injected;

let virtual = false;

/** Marks this isolate's `node:fs` as a virtual disk: no thread may walk the machine's real one in its place. */
export function useVirtualDisk(): void {
  virtual = true;
}

export const isVirtualDisk = (): boolean => virtual;
