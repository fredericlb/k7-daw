# Navigating a media pane

`src/panZoom.ts` is the navigation the media panes share. They scale to **FIT** by default, so
there is nothing to scroll until you zoom: `useHZoom` scales horizontally, anchored so the point
under the cursor stays put, and `usePanScroll` pans on a MIDDLE-mouse drag. It is unit-free on
purpose — the caller multiplies its own lane width, in beats or in seconds. The arithmetic (the wheel's factor, the
ceiling, the scroll that keeps the anchor) is the `gesture` crate's (`apps/k7-daw-wasm/gesture/src/pan_zoom.rs`).

Space is not a pan modifier, because Space is the transport toggle in every app that mounts these.

## Only a pinch or Cmd/Ctrl zooms; a bare wheel scrolls

`wheelZooms` is the whole rule, in one place: a wheel event zooms when the browser marks it as a
pinch (`ctrlKey`) or Cmd is held (`metaKey`), and every other wheel event — a mouse notch, a
trackpad slide, Shift+wheel — is handed straight back to the browser. That is the convention of
every DAW and video editor a newcomer has met: the wheel walks the lanes vertically, Shift+wheel
moves sideways, and zooming is asked for. A bare wheel that zoomed left a mouse with no way down a
song of ten lanes but the scrollbar, and nothing on screen said so.

The browser's native scroll is the pan, with the momentum, the axis locking and the rubber-banding a
hand-rolled `scrollLeft` would have to counterfeit, on both axes at once.

`isZoomWheel` is the other half. On a Mac trackpad — and on a Windows precision touchpad — a PINCH
arrives as a wheel event with `ctrlKey` set, synthesised by the browser, while two fingers sliding
arrive as a plain wheel. `metaKey` joins it because Cmd+scroll is the same intention on a Mac mouse.

## Telling a wheel from a hand — the chains graph

The graph has no lanes to walk, so a mouse wheel zooms there (`ui-chains-react`'s camera), while a
trackpad slide still pans it. `isMouseWheel` is what tells the two apart, and lives here beside
`isZoomWheel`.

`isMouseWheel` reads three signals, most reliable first:

1. **`deltaMode` other than pixels** — lines or pages. Only a real wheel is ever reported that way
   (Firefox does it for mice), so it settles the question on its own.
2. **`wheelDeltaY`**, the legacy field Chrome and Safari still carry. A mouse reports it in
   multiples of 120 — the detent, from the Windows API that named it — while a trackpad reports
   three times its pixel delta, which lands on 120 only by accident. This is what separates a Mac
   trackpad from a Mac mouse, and nothing standard does.
3. Failing both (Firefox in pixel mode), **the shape of the delta**: a notch is integral, has no
   horizontal component, and is LARGE, where a finger's is small and often fractional
   (`MOUSE_NOTCH_MIN`).

It is **wrong in one direction only, by design**: an unrecognised device is treated as a surface, so
it scrolls. A trackpad that zooms is a broken trackpad; a mouse that pans is merely a mouse that
does not zoom yet.

## A notch and a pinch cannot share an answer

A mouse wheel arrives as ONE event per detent, with a large delta nobody reads the size of, so a
fixed step per event (`WHEEL_STEP`) is exactly right. A trackpad PINCH arrives as Ctrl+wheel: a
stream of small continuous deltas, dozens a second. Given the notch's fixed step, that stream
multiplies 1.15 per event and slams into the zoom limit before the fingers have finished moving —
which is how "pinch does not work" is really spelled. Scaled BY the delta, it tracks the fingers.

The delta is clamped first: a browser that reported a pinch in hundreds rather than in units would
otherwise jump a decade in one event.

Inside the handler the test is `e.ctrlKey` and **not** `isZoomWheel(e)`. The guard says WHETHER to
zoom, this says how much, and the two disagree on purpose: a ctrl-wheel is a trackpad pinch, while
Cmd+wheel is a Mac mouse, one notch per detent. Both reach there; only the first is a pinch. The
dominant axis wins, because a pinch is `deltaY` but Ctrl held on a horizontal wheel is still
someone asking to zoom.

## The ceiling is derived, not chosen

`MAX_LANE_PX` is where the limit actually lives. A browser refuses to lay out an element past about
33.5 million pixels, and a lane that asked for more would be silently clipped — `scrollWidth` would
stop agreeing with the width the caller computed, and the anchored zoom would drift a little
further off with every notch. Half of that, so the margin is the browser's own rounding rather than
a guess.

`zoomCeiling` is derived rather than a flat `max: 32`. Thirty-two is a number, not a reason — and
it is the one people MEET: past it the wheel simply stops, with nothing on screen to say why.
Derived, the ceiling is about 13 000× on a laptop pane, which is one 64th of a beat filling the
screen on a five-minute song. Nobody meets that either; what they meet is the material.

The tick layers window themselves to the pane (`apps/k7-daw-frontend/src/components/Ruler.tsx`'s
`useVisibleBeats`), which is what makes this number the only ceiling left: a whole-song list
saturates at 64 nodes per beat, and that would be the real one.

`min` is 1 and stays 1, and that is not the same kind of bound: **1 IS fit**, so below it the pane
would draw the whole thing plus empty space. There is nothing further out to see.

The pane's width is measured by the hook's own `ResizeObserver` rather than taken as a prop: the
hook already holds the element, and a caller measuring it again would be a second answer to one
question. It is remeasured on resize because a window dragged narrower can only be zoomed further,
not less.

## Anchoring, and why the scroll is deferred

`zoomAt` keeps the content under the cursor fixed: it scales that point's x by the zoom ratio and
offsets the scroll, with the gutter — which does not scale — subtracted and re-added around it. The
new `scrollLeft` is stored and applied in a layout effect AFTER the width change renders: computed
on the old width, correct on the new.

It is exposed because the wheel is not the only gesture that zooms: k7's ruler zooms on a vertical
drag and calls this same function rather than copying the arithmetic. Applied straight away instead
of deferred, the new `scrollLeft` is clamped by the old `scrollWidth`, so an anchored zoom drifts.
One implementation, both gestures.

`preventDefault` on the wheel stops the page moving *and* stops the browser zooming the whole PAGE
on the same gesture — its own Ctrl+wheel zoom is exactly the gesture we just claimed. Which is why
the listener is `passive: false`; a plain wheel returns before that and scrolls as usual.

## Two fingers on a touchscreen

**An editing surface moves under two fingers only; one finger moves nothing.** Its clips, notes and
cards are what a finger lands on, so a surface that scrolls under one finger reads every stray swipe
either as a pan or as a drag of whatever it began on. With the count of fingers deciding, one finger
is a tap or a hold (`hold.ts`, and `apps/k7-daw-frontend/docs/timeline.md`, "A finger holds, then
drags"), and two are the view.

`useTwoFingers(ref, { onStart, onMove(start, frame) })` is the one hook for it (`twoFingers.ts`):
`onStart` takes the baseline (a zoom and a scroll, a camera, a bar window) or refuses the gesture,
`onMove` gets the midpoint's travel and the spread's ratio, both against the START. `useHZoom` mounts
it, and `useTwoFingerScroll` is the same for a container that only scrolls. Each surface brings its
own clamp.

- **`touch-action: none` is set by the hook**, not left to each caller's stylesheet: one finger is
  not the browser's to scroll with. A list nested in the surface still scrolls under one finger,
  since the property stops at the nearest scroll container, which is why the hook cancels a
  `touchmove` only while two fingers are its own.
- **Measured from the start, not frame to frame**: a ratio of ratios accumulates rounding, and
  pinching out and back in would not return where it began. The pan is the same — a scroll from
  the one the gesture began on — so the zoom anchors where the fingers LANDED and the pan carries
  that point to where they are.
- **The zoom waits for 4% of spread** (`PINCH_SLOP`): a pan whose fingers waver would otherwise
  re-lay a whole timeline every frame for a zoom nobody asked for.
- **Touch events rather than pointer events**: the pointer flow is where each finger's own drag
  lives, and a gesture of two fingers read there would be two drags.
- **A drag already armed by a hold owns the touch**: no two-finger gesture starts under it
  (`holdTwoFingers`), and a second finger landing BEFORE the hold cancels the hold instead.

There is no momentum: the fingers are the motion, and a glide would be a counterfeit of the
browser's.

## Middle-drag panning

Panning moves the view under the cursor — dragging right reveals content to the left — on whichever
axis actually overflows, so a fit-width lane can still be panned vertically and a zoomed one
horizontally.

The pointer-down handler takes the gesture with both `preventDefault` and `stopPropagation`: no
autoscroll, and no child handler (the roll's note-draw) claiming the press first. The middle-click
autoscroll puck is a `mousedown` default, so it is cancelled there too.

Both listeners are on the **capture** phase: a bubble listener never fires if a child stops
propagation, and it loses the autoscroll race on Firefox. Non-middle buttons return early WITHOUT
stopping the event, so child clicks still work.

## The graph shares the rule and the pinch

The chains graph (`apps/k7-daw-frontend/packages/ui-chains-react/src/camera.ts`) answers the
wheel-vs-trackpad question the same way and takes its pinch from `wheelFactor` and `useTwoFingers` here.
It keeps its own NOTCH rate — a detent scales by its delta there, where a pane takes a step — and
its own camera arithmetic, because a pane here scrolls and the graph translates. The rule is
recorded in `docs/inventaire.md` and tested on both sides so it cannot drift in silence.

## One wheel for every pane

A bare wheel scrolls, in every pane that draws a window rather than a scroller too: the take panel
(`AudioClipPanel`) slides its bar window under a bare wheel and zooms only on a pinch or Cmd/Ctrl.
