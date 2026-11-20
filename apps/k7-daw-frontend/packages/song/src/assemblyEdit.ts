// assemblyEdit.ts — making a box, and taking one apart. Instancing copies the template's node
// files into the project and its segments into `graph.k7x`, so what lands on disk is an ordinary
// graph plus one record saying which of its nodes are that box: nothing downstream learns a thing.
// The three writes that keep the record honest when a node is renamed or deleted are here too.
// Full rationale: docs/assemblies.md.

import { stub } from "./editStub";

/** The name a member takes in the project: the box's, then the template's own word for it. */
export const memberName = (assembly: string, member: string): string =>
  `${assembly}_${member}`;

/** Instance a template into this project — its node files, its segments, and the record. */
export function createAssembly(
  dir: string,
  ref: string,
  id: string,
  at?: { x: number; y: number },
): { file: string; members: Record<string, string> } {
  return stub("song/assemblyEdit.createAssembly", dir, ref, id, at);
}

/** The members of a box, as node names — what `assembly delete` walks and `dissolve` leaves alone. */
export function assemblyMembers(dir: string, id: string): string[] {
  return stub("song/assemblyEdit.assemblyMembers", dir, id);
}

/** Drop the RECORD and nothing else: the nodes stay exactly where they are, wired as they were. */
export function dissolveAssembly(dir: string, id: string): string {
  return stub("song/assemblyEdit.dissolveAssembly", dir, id);
}

/** A node was RENAMED: every box holding it follows, since a member is a reference we can write back. */
export function renameNodeInAssemblies(
  dir: string,
  from: string,
  to: string,
): string[] {
  return stub("song/assemblyEdit.renameNodeInAssemblies", dir, from, to);
}

/** …and a node DELETED: the member goes, and the box with it once it has no members left. */
export function dropNodeFromAssemblies(dir: string, node: string): string[] {
  return stub("song/assemblyEdit.dropNodeFromAssemblies", dir, node);
}
