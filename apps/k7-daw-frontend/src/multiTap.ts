// multiTap.ts — a double tap with two fingers or three, the touch shell's undo and redo. A tap is every finger down and
// up again quickly without moving; two of them with the same finger count, close together, are the gesture.
// The one thing a reader would get wrong: a pinch or a two-finger scroll moves, so it is never a tap (docs/shell.md).

/** Longest a tap may last, from the first finger down to the last up. */
export const TAP_MS = 300;
/** Longest gap between the first tap's end and the second's start. */
export const DOUBLE_GAP_MS = 400;
/** How far a finger may drift and still be tapping. */
export const TAP_SLOP_PX = 12;

export interface TapPoint {
  id: number;
  x: number;
  y: number;
}

/** One touch sequence's recogniser: fed the browser's touches, it calls `onDouble` with 2 or 3 when the gesture lands. */
export class MultiTap {
  private g: {
    t0: number;
    max: number;
    start: Map<number, { x: number; y: number }>;
    moved: boolean;
  } | null = null;
  private last: { count: number; at: number } | null = null;

  constructor(
    private readonly onDouble: (fingers: 2 | 3) => void,
    private readonly now: () => number = () => performance.now(),
  ) {}

  /** Fingers went down; `all` is every finger now on the glass. */
  down(added: TapPoint[], all: number): void {
    if (!this.g)
      this.g = { t0: this.now(), max: 0, start: new Map(), moved: false };
    for (const p of added) this.g.start.set(p.id, { x: p.x, y: p.y });
    this.g.max = Math.max(this.g.max, all);
  }

  /** Fingers moved: past the slop, this sequence is a scroll or a pinch, not a tap. */
  move(points: TapPoint[]): void {
    const g = this.g;
    if (!g || g.moved) return;
    for (const p of points) {
      const s = g.start.get(p.id);
      if (s && Math.hypot(p.x - s.x, p.y - s.y) > TAP_SLOP_PX) g.moved = true;
    }
  }

  /** Fingers lifted; `left` is how many are still down. The sequence ends when none are. */
  up(left: number): void {
    const g = this.g;
    if (!g || left > 0) return;
    this.g = null;
    const end = this.now();
    const count = g.max;
    const tap =
      !g.moved && end - g.t0 <= TAP_MS && (count === 2 || count === 3);
    if (!tap) {
      this.last = null;
      return;
    }
    const prev = this.last;
    if (prev && prev.count === count && g.t0 - prev.at <= DOUBLE_GAP_MS) {
      this.last = null;
      this.onDouble(count as 2 | 3);
      return;
    }
    this.last = { count, at: end };
  }

  /** The browser took the touches away: nothing counts. */
  cancel(): void {
    this.g = null;
    this.last = null;
  }
}
