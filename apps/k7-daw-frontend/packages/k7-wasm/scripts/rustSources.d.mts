// rustSources.d.mts — the types of rustSources.mjs, for the test that imports it.
export const CRATES: string[];
export const INCLUDED: string[];
export function rustSourceHash(root?: string): string;
export function typesHash(): string;
export function pathDeps(): string[];
