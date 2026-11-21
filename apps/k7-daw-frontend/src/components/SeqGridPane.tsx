// SeqGridPane.tsx — a sequence's piano roll where the chains graph normally is,
// opened by `sequence edit roll <sequence>` and closed by the × back to the graph.
// The frame and its bar; the file behind it is `SeqGridFile`.
// See docs/noteGrids.md.

import { Piano } from "@studio/ui/icons";
import { useEffect, useState } from "react";
import { api } from "../api";
import { m } from "../paraglide/messages.js";
import { fileStem } from "../projectPaths";
import type { Hints } from "../types";
import { GraphSlotPane } from "./GraphSlotPane";
import { SeqGridFile } from "./SeqGridFile";

export function SeqGridPane({
  path,
  onChanged,
  setStatus,
  reloadKey,
}: {
  path: string;
  // A write landed: the polls need to pick the notes up (the timeline draws them too).
  onChanged: () => void;
  setStatus: (s: string) => void;
  /** Handed to the grid, which re-reads its file on it. */
  reloadKey?: number;
}) {
  const [racks, setRacks] = useState<Hints["racks"]>({});

  useEffect(() => {
    let done = false;
    api.hints().then((h) => !done && setRacks(h.racks));
    return () => {
      done = true;
    };
  }, []);

  return (
    <GraphSlotPane
      // Its own strip: a glyph, the stem, and the path in full .
      bar={
        <>
          <Piano className="h-4 w-4 text-muted-foreground" />
          <span className="font-medium text-foreground">{fileStem(path)}</span>
          <span>{m["seq.pianoRoll"]()}</span>
          <span className="truncate">{path}</span>
        </>
      }
    >
      <div className="h-full min-h-0 overflow-auto">
        <SeqGridFile
          path={path}
          racks={racks}
          onChanged={onChanged}
          setStatus={setStatus}
          reloadKey={reloadKey}
        />
      </div>
    </GraphSlotPane>
  );
}
