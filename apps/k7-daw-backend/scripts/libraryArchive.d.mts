// libraryArchive.d.mts — the type of libraryArchive.mjs, for the TypeScript that imports it (file-hub's test).

/** Writes into `dest` the `k7-core` this build carries, manifest included, from the repo at `repo`. */
export function bundleLibrary(repo: string, dest: string): Promise<void>;
