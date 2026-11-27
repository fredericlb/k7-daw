// browser.tsx — the desk's drawer. It takes the three things a ring slot has (surface, deck,
// project) plus the revision token the shell injects, fetches its own graph, and draws the desk.
// It imports NO shell code — the deps below are all packages.

import type { WebGraph } from "@studio/graph-model";
import type { ProjectSurface } from "@studio/k7-wasm/types";
import { projectApi } from "@studio/project-shell/project";
import { useEffect, useMemo, useState } from "react";
import type { Deck } from "./engine";
import { useDeckSet } from "./useDeckSet";
import { Turntables } from "./views/Turntables";

/** The props the drawer takes — a structural SUBSET of both mounts' ctx (the ring slot's and a
 *  device's), which is what makes one drawer legal in both (apps/k7-daw-frontend/packages/k7-plugins/docs/plugins.md). */
export interface TurntablesPaneProps {
  surface: ProjectSurface;
  deck: Deck;
  /** Workspace-relative, as every file route takes it. */
  project: string;
  /** The project's change token; the graph is refetched whenever it moves. */
  rev: string | null;
}

export function TurntablesPane(props: TurntablesPaneProps) {
  const { surface, deck, project, rev } = props;
  // The compiled chains, fetched here rather than taken from the shell — `App`'s `seedGraph` is a command's answer, not the current graph.
  const [graph, setGraph] = useState<WebGraph | null>(null);
  useEffect(() => {
    if (rev === null) return;
    let alive = true;
    projectApi
      .graph()
      .then((g) => alive && setGraph(g))
      // A project that will not compile still has a desk worth drawing: the bindings say so, and it is the screen you fix it from.
      .catch(() => alive && setGraph(null));
    return () => {
      alive = false;
    };
  }, [rev]);

  /** The mapping, held stable across renders: `useDeckSet` takes it as an effect dependency. */
  const mapping = useMemo(
    () => surface.surface.mapping,
    [surface.surface.mapping],
  );

  /** The playlist the declaration names — a plain string off the surface, so it needs no memo of its own. */
  const playlist = surface.surface.playlist;

  const set = useDeckSet(deck, project, graph, rev ?? "", mapping, playlist);

  // No `open` branch and no chrome of its own: the pane wrapper in `App` carries the border, the ring and the `hidden`.
  return (
    <div className="flex min-h-0 min-w-0 flex-1 flex-col">
      <Turntables
        deck={deck}
        set={set}
        mapping={mapping}
        playlist={playlist}
        file={surface.file}
      />
    </div>
  );
}
