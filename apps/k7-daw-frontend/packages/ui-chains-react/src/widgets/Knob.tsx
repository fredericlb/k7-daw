// Knob.tsx — a knob dial as an SVG arc, its reading in the middle and a curve's span inside the
// value's ring — docs/widgets.md.

import { arcPath } from "@studio/ui/knob";
import { MAUVE, SAND } from "../chainsCss";

// A knob dial as an SVG arc (the kit's arcPath), 0..1, with `text` read in its middle — docs/widgets.md.
export function Knob({
  t,
  known,
  size,
  text,
  span,
}: {
  t: number;
  known: boolean;
  size: number;
  text?: string;
  /** The stretch of the travel a curve sweeps, drawn in mauve inside the value's ring. */
  span?: [number, number];
}) {
  const START = Math.PI * 0.75,
    SWEEP = Math.PI * 1.5;
  const c = size / 2;
  const R = c - 2.5;
  const pt = (a: number) => [c + Math.cos(a) * R, c + Math.sin(a) * R];
  // The depth gauge rides just inside the value's ring, so both read at once.
  const depthR = R - 3;
  const tt = Math.max(0, Math.min(1, t));
  const a = START + SWEEP * tt;
  const [ix, iy] = pt(a);
  // The pointer starts here rather than at the centre, leaving the middle to the reading.
  const tickR = R * 0.72;
  const [tx, ty] = [c + Math.cos(a) * tickR, c + Math.sin(a) * tickR];
  const font = Math.max(6, Math.round(size * 0.2));
  // Monospace advance is ~0.6em; the -1 keeps a hair between the last glyph and the pointer.
  const readable = !!text && text.length * 0.6 * font <= 2 * (tickR - 1);
  return (
    <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
      <circle cx={c} cy={c} r={c - 1} fill="#0e0e12" stroke="#26262d" />
      <path
        d={arcPath(R, 0, 1, c)}
        fill="none"
        stroke="#2f2f36"
        strokeWidth="2.2"
        strokeLinecap="round"
      />
      {span && span[1] > span[0] && (
        <path
          d={arcPath(depthR, span[0], span[1], c)}
          fill="none"
          stroke={MAUVE}
          strokeWidth="2"
          strokeLinecap="round"
        />
      )}
      {known && (
        <>
          <path
            d={arcPath(R, 0, tt, c)}
            fill="none"
            stroke={SAND}
            strokeWidth="2.2"
            strokeLinecap="round"
          />
          <line
            x1={tx}
            y1={ty}
            x2={ix}
            y2={iy}
            stroke="#e7e6ea"
            strokeWidth="1.5"
            strokeLinecap="round"
          />
        </>
      )}
      {readable && (
        <text
          x={c}
          y={c}
          textAnchor="middle"
          dominantBaseline="central"
          fill={known ? "#e7e6ea" : "var(--faint)"}
          fontSize={font}
          fontFamily="ui-monospace, monospace"
          style={{ fontVariantNumeric: "tabular-nums" }}
        >
          {text}
        </text>
      )}
    </svg>
  );
}
