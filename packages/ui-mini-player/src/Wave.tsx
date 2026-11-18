// Wave.tsx — the mini player's background: two slow waves that drift faster as what plays gets louder.
// The level is a FUNCTION sampled in the frame loop, never a value held in state: a meter at 20 Hz
// would re-render the bar for a picture. Drawn as SVG paths in a 100x100 box stretched over the bar,
// so it needs no canvas sizing and takes its colour from the theme. See ../docs/miniPlayer.md.

import { useEffect, useRef } from "react";

/** Cycles per second at silence, and at full scale: the whole point of the wave is that it is slow. */
const SLOWEST = 0.05;
const FASTEST = 0.55;
/** How fast the speed itself follows the level, in the loop — a meter jumps, a swell does not. */
const EASE_PER_S = 2;
/** Frames per second: a background needs no more, and this bar is on screen the whole time. */
const FPS = 30;
/** Points per layer, enough for one smooth crest across the widest window. */
const POINTS = 40;

interface Layer {
  /** How tall the crest runs, in hundredths of the bar's height. */
  amplitude: number;
  /** Crests across the bar: the two layers never share a rhythm. */
  cycles: number;
  /** Where it starts, so the two are out of step from the first frame. */
  offset: number;
  /** +1 drifts rightward, -1 leftward. */
  direction: 1 | -1;
  opacity: number;
  /** Filled down to the bar's foot, or a line alone. */
  filled: boolean;
}

const LAYERS: Layer[] = [
  {
    amplitude: 30,
    cycles: 1.5,
    offset: 0,
    direction: 1,
    opacity: 0.08,
    filled: true,
  },
  {
    amplitude: 22,
    cycles: 2.25,
    offset: 0.35,
    direction: -1,
    opacity: 0.18,
    filled: false,
  },
];

/** One layer's path at that phase: a sine across the 100-unit box, closed down to the foot when filled. */
function crest(phase: number, layer: Layer): string {
  const points: string[] = [];
  for (let i = 0; i <= POINTS; i++) {
    const x = (i / POINTS) * 100;
    const turn = ((x / 100) * layer.cycles + phase) * Math.PI * 2;
    const y = 50 + Math.sin(turn) * layer.amplitude;
    points.push(`${i === 0 ? "M" : "L"}${x.toFixed(2)} ${y.toFixed(2)}`);
  }
  if (layer.filled) points.push("L100 100L0 100Z");
  return points.join("");
}

/** The bar's background; `level` answers 0..1 for whatever plays, and is absent where nothing measures it. */
export function MiniPlayerWave({ level }: { level?: () => number }) {
  const svg = useRef<SVGSVGElement>(null);

  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const paths = Array.from(el.querySelectorAll("path"));
    const phases = LAYERS.map((l) => l.offset);
    const speeds = LAYERS.map(() => SLOWEST);
    const draw = () => {
      for (let i = 0; i < LAYERS.length; i++)
        paths[i]?.setAttribute("d", crest(phases[i], LAYERS[i]));
    };
    // Nothing moves for someone who asked the system for that: one frame, and it stays.
    if (
      typeof matchMedia === "function" &&
      matchMedia("(prefers-reduced-motion: reduce)").matches
    ) {
      draw();
      return;
    }
    let raf = 0;
    let last = 0;
    const step = (now: number) => {
      raf = requestAnimationFrame(step);
      if (now - last < 1000 / FPS) return;
      // Clamped, so a tab brought back to the front does not jump the crests a minute forward.
      const dt = last ? Math.min(0.25, (now - last) / 1000) : 0;
      last = now;
      const gain = Math.max(0, Math.min(1, level?.() ?? 0));
      for (let i = 0; i < LAYERS.length; i++) {
        const target = SLOWEST + (FASTEST - SLOWEST) * gain;
        speeds[i] += (target - speeds[i]) * Math.min(1, dt * EASE_PER_S);
        phases[i] += dt * speeds[i] * LAYERS[i].direction;
      }
      draw();
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [level]);

  return (
    <svg
      ref={svg}
      aria-hidden="true"
      viewBox="0 0 100 100"
      preserveAspectRatio="none"
      className="pointer-events-none absolute inset-0 h-full w-full text-primary-text"
    >
      {LAYERS.map((layer) => (
        <path
          key={`${layer.cycles}-${layer.direction}`}
          opacity={layer.opacity}
          fill={layer.filled ? "currentColor" : "none"}
          stroke={layer.filled ? "none" : "currentColor"}
          // In screen pixels, or the box's stretch would draw a different line on every width.
          vectorEffect="non-scaling-stroke"
          strokeWidth={1.2}
        />
      ))}
    </svg>
  );
}
