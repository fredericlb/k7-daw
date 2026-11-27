// rangeGesture.ts — the commit rule of a range: a gesture commits the last value the user set, once, and never without one.
// Blur, key up and pointer up all end a gesture; ending one that changed nothing commits nothing.

export function rangeGesture(commit: (v: number) => void) {
  let last: number | null = null;
  return {
    /** The user moved the control to `v`. */
    change(v: number) {
      last = v;
    },
    /** A gesture ended: commit what was set, if anything. */
    end() {
      if (last === null) return;
      const v = last;
      last = null;
      commit(v);
    },
  };
}
