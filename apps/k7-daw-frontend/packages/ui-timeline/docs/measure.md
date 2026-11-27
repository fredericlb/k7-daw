# A pane is measured once (`src/measure.ts`)

Every surface here needs its scroller's size, and a few its scroll position. Each used to write the
same `ResizeObserver` effect, and each wrote it slightly differently — a width in one, a width and a
height in another, a re-read on scroll in a third — so one pane could answer "how wide am I?" twice,
two ways.

- **`useElementWatch(ref, read, { scroll })`** is the primitive: `read` runs now, on every resize of
  the element and, with `scroll`, on every scroll. It is for a reading that must NOT re-render the
  host per event — the roll's culling window and the timeline's clip window change in half-viewport
  steps and set state only then.
- **`useElementSize(ref)`** is the client width and height as state, 0 until the element exists.
- **`useScrollBox(ref)`** is the scroll offset and the visible width as state. It re-renders on every
  scroll, so it belongs to a leaf (the ruler's tick layer), never to a surface that owns the scroller.

The `deps` list re-binds the watch for a pane whose scroller mounts late (it renders "loading…"
first), as `useHZoom`'s does.

**A pane's width is measured once.** `useHZoom` measures the container for its zoom ceiling and hands
the width back as the fourth element of its result, and the timeline reads its own layout from that
value rather than observing the scroller a second time; `gutter` may be a function of that width for a
name column that grows with the pane.
