// trackDelete.ts — `track delete <track>`: the lane AND the chain it starts — input, instrument,
// effects, strip — which is what deleting a track means in any DAW; a lane gone with its instrument
// left wired would sound under nothing. Its clips and sequences stay on disk, as `event remove`
// leaves them, and the journal undoes the lot. The catalogue marks it `confirm:`, so the menu asks.

import { isChainProject, readChainProject } from "@studio/song/chainProject";
import { stubBatch } from "@studio/song/editStub";
import { loadGraph } from "@studio/song/graph";
import { ownLaneStrip } from "@studio/song/laneStrip";
import { loadProjectRoot } from "@studio/song/loader";
import { surfacesNaming } from "@studio/song/surfaces";
import { entityFileFor } from "@studio/song/yaml/moduleIO";
import type { CommandHost, SidHandler } from "../commands.ts";
import { splitWords } from "./shared.ts";

export const TRACK_DELETE_VERBS: Record<string, SidHandler> = {
  "track.delete": trackDeleteCommand,
};

/** The nodes a lane takes with it, the lane LAST: each delete rejoins around its node, and the lane is what the chain hangs on. */
function laneOwnNodes(dir: string, lane: string): string[] {
  if (isChainProject(dir)) {
    const cp = readChainProject(dir);
    const chain = cp.nodes[lane]?.chain;
    if (!chain) return [lane];
    const others = Object.values(cp.nodes)
      .filter((n) => n.chain === chain && n.name !== lane)
      .map((n) => n.name);
    return [...others, lane];
  }
  // A graph.k7x project names no chain a lane owns: its strip goes with it, every other node is shared.
  const mixer = ownLaneStrip(dir, loadGraph(dir), lane);
  return mixer ? [mixer, lane] : [lane];
}

async function trackDeleteCommand(a: string, h: CommandHost): Promise<void> {
  const arg = splitWords(a)[0];
  if (!arg) {
    h.refuse(["usage: track delete <track>"]);
    return;
  }
  const p = h.project();
  const t = p.tracks.find((x) => x.id === arg || x.name === arg);
  if (!t) {
    h.refuse([`no track "${arg}"`]);
    return;
  }
  const dir = h.songPath;
  // A surface mapping names the lane and nothing rewrites one, so deleting would leave it pointing nowhere.
  const mapped = surfacesNaming(dir, t.id);
  if (mapped.length) {
    h.refuse([
      `"${t.id}" is named by ${mapped.length > 1 ? "surfaces" : "surface"} ${mapped.join(", ")} — unmap it there first`,
    ]);
    return;
  }
  try {
    const nodes = laneOwnNodes(dir, t.id);
    stubBatch(
      nodes.flatMap((node) => [
        ["song-edit/graphEdit.deleteChainNode", dir, node],
        ["song/assemblyEdit.dropNodeFromAssemblies", dir, node],
      ]),
    );
    // project.k7x names lanes in three lists; a deleted one would be named there for ever.
    const root = loadProjectRoot(dir);
    const projectFile = entityFileFor(dir, "", "project");
    const edits: [string, ...unknown[]][] = [];
    for (const key of ["trackOrder", "soloed", "highlighted"] as const) {
      const list = (root as Record<string, unknown>)[key];
      if (!projectFile || !Array.isArray(list) || !list.includes(t.id))
        continue;
      const next = list.filter((x) => x !== t.id);
      edits.push(
        next.length
          ? ["song/yaml/edit.setFileValue", projectFile.path, [key], next]
          : ["song/yaml/edit.deleteFileValue", projectFile.path, [key]],
      );
    }
    stubBatch(edits);
    h.log([
      `deleted ${t.id} and the ${nodes.length - 1} node(s) of its chain — its clips stay on disk`,
    ]);
    await h.reload();
  } catch (e) {
    h.refuse([`track delete: ${(e as Error).message}`]);
  }
}
