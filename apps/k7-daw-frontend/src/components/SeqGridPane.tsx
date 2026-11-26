// SeqGridPane.tsx — a sequence's piano roll where the chains graph normally is,
// opened by `sequence edit roll <sequence>` and closed by the × back to the graph.
// The frame and its bar; the file behind it is `SeqGridFile`.
// See docs/noteGrids.md.

import { useEffect, useState } from "react";
import { api } from "../api";
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
    <GraphSlotPane>
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
