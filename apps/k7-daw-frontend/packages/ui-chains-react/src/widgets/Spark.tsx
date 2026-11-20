// Spark.tsx — a curve card's picture: the curve editor's preview on the same -1..1 frame, with
// the transport's playhead while the song plays — docs/nodeCard.md.

import type { CurveSpec } from "@studio/graph-model";
import { CurveHandle, K7WasmError } from "@studio/k7-wasm/curve";
import { useContext, useEffect, useMemo } from "react";
import { MAUVE } from "../chainsCss";
import { SongClockContext, usePlayingBeat } from "../chainsStores";

/** The playhead of an open curve, none for a refused one. */
function playheadOf(handle: CurveHandle | null, beat: number | null) {
  if (!handle || beat === null) return null;
  try {
    return handle.playhead(beat);
  } catch (e) {
    if (e instanceof K7WasmError) return null;
    throw e;
  }
}

// A curve card's picture: the curve editor's own preview, on the same -1..1 frame, its next cycle faded — docs/nodeCard.md.
export function Spark({ spec }: { spec?: CurveSpec }) {
  const clock = useContext(SongClockContext);
  const num = clock?.num ?? 4;
  const den = clock?.den ?? 4;
  const beat = usePlayingBeat(clock?.tempo);
  // A unit box stretched to the card's body; the strokes do not scale with it (`non-scaling-stroke`).
  const W = 100,
    H = 100,
    p = 4;
  // One curve held open per spec: the picture once, then only the playhead each frame.
  const handle = useMemo(
    () => (spec ? new CurveHandle(spec, num, den) : null),
    [spec, num, den],
  );
  useEffect(() => () => handle?.close(), [handle]);
  const drawn = useMemo(() => {
    if (!handle) return null;
    try {
      const { solid, faded, beats } = handle.preview;
      const xy = (q: { x: number; y: number }) =>
        `${(p + q.x * (W - 2 * p)).toFixed(1)} ${(H / 2 - q.y * (H / 2 - p)).toFixed(1)}`;
      const line = (qs: { x: number; y: number }[]) =>
        qs.length ? `M ${qs.map(xy).join(" L ")}` : "";
      return { solid: line(solid), faded: line(faded), beats };
    } catch (e) {
      // A curve the module refuses draws the empty frame.
      if (e instanceof K7WasmError) return null;
      throw e;
    }
  }, [handle]);
  // Where the transport is on the picture, and the value the curve has there — in % of the box, the SVG's own units.
  const head = playheadOf(handle, drawn ? beat : null);
  const left = head ? p + head.x * (W - 2 * p) : 0;
  const top = head ? H / 2 - head.y * (H / 2 - p) : 0;
  return (
    <div style={{ position: "relative", width: "100%", height: "100%" }}>
      <svg
        width="100%"
        height="100%"
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        style={{ display: "block" }}
      >
        {/* Every beat, faint, and the bars over them — the editor's grid. */}
        {drawn?.beats.map((b) => (
          <line
            key={b.x}
            x1={p + b.x * (W - 2 * p)}
            x2={p + b.x * (W - 2 * p)}
            y1={p}
            y2={H - p}
            stroke="#3a3842"
            strokeWidth={b.bar ? 0.75 : 0.5}
            strokeOpacity={b.bar ? 1 : 0.5}
            vectorEffect="non-scaling-stroke"
          />
        ))}
        {/* 1 and -1, and 0 dashed between them — the editor's frame. */}
        {[p, H - p].map((y) => (
          <line
            key={y}
            x1={p}
            x2={W - p}
            y1={y}
            y2={y}
            stroke="#3a3842"
            vectorEffect="non-scaling-stroke"
            strokeWidth="0.75"
          />
        ))}
        <line
          x1={p}
          x2={W - p}
          y1={H / 2}
          y2={H / 2}
          stroke="#3a3842"
          vectorEffect="non-scaling-stroke"
          strokeWidth="0.75"
          strokeDasharray="2 2"
        />
        {drawn && (
          <>
            <path
              d={drawn.solid}
              fill="none"
              stroke={MAUVE}
              vectorEffect="non-scaling-stroke"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
            {drawn.faded && (
              <path
                d={drawn.faded}
                fill="none"
                stroke={MAUVE}
                vectorEffect="non-scaling-stroke"
                strokeWidth="1.5"
                strokeOpacity={0.3}
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            )}
          </>
        )}
      </svg>
      {head && (
        <>
          <i className="dgx-curvehead" style={{ left: `${left}%` }} />
          <i
            className="dgx-curvedot"
            style={{ left: `${left}%`, top: `${top}%` }}
          />
        </>
      )}
    </div>
  );
}
