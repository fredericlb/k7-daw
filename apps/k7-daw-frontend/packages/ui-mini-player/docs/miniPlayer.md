# The bar, and nothing else

`MiniPlayer` is a **view**. It holds no `<audio>` element, no socket, no timer and no transport
state: `playing`, `position` and `duration` are props, and the three buttons are callbacks. The one
thing it keeps is where a scrubber being dragged is (below). The screens
that mount it play things that have nothing in common —

| | k7's references | a release's page | a k7 project |
|---|---|---|---|
| what plays | a reference track in the workspace | a published version | the arrangement |
| through what | this tab's `<audio>`, always | this tab's `<audio>` | the engine, over its socket |
| beside what | the engine playing the project, unbothered | the waveform player it drives | a reference track, which keeps the system's controls |

— so a bar that owned a transport could serve exactly one of them. What is shared is the row: a
play/pause on the left, the title with whatever the host has to say about it, a clock either side
of a scrubber, the host's own controls, and a close top right.

## One height, whatever is in it

**40 px with a mouse, 60 px on a device without hover, everywhere and always** — the height k7's
project bar has with 24 px buttons, and with the 44 px slots a phone gives its way home. The bar is
at the foot of every page, so a height that followed its content would move the page under it each
time a song started or ended: a song with a waveform is a strip, one without is a title and a
scrubber, and a dock's standby bar holds a title alone.

That height is also the reason **the title and the scrubber share one row**: a title line above a
scrubber is 44 px of content, which no reading of the bar's height survives. The title is capped at
half the row and truncates there; the scrubber takes the rest and its two clocks.

## The wave behind the row

A slow wave drifts behind everything in the bar, in the app's violet at low opacity — two layers
moving in opposite directions so they never settle into one line. It **speeds up with the gain of
what plays**: `level` is a getter a host passes, in 0..1, asked once per drawn frame, and the wave
eases toward the speed it names rather than jumping to it. A host that measures nothing passes
nothing, and the wave keeps its slowest crawl — a wave that stopped would read as a broken picture,
and one that reacted to a guess would read as a lie.

It is a getter and not a value for the reason the engine's state is a store: a host's meter arrives
at 20 Hz, and a prop would re-render the whole row twenty times a second to move a crest. Nothing
here re-renders at all — the paths are set on the SVG in the loop.

It is drawn as SVG in a 100×100 box stretched over the bar, not on a canvas: it needs no pixel
sizing, stays crisp at any width, takes its colour from the theme's text, and the stroke is
`non-scaling` so the line is the same weight on a bar 400 px wide and one 2000 px wide. The bar's
own boxes are positioned, so they paint over it; and the bar keeps no `overflow` of its own, since
the minimap's peek grows up out of it. A reader who asked their system for reduced motion gets it
drawn once and left still.

## `clock` is exported, and it is the point of the file being shared

Two readings of one transport that round differently is a bug nobody reports and everybody sees.
One function, one bar.

## Every string is a prop

`labels` carries the five: the region's accessible name, play, pause, the scrubber and the close.
Not a default in English with an override — a default is what ships untranslated. The hosts
translate through their own catalogues, so the bar must have no opinion.

## Three slots, because a title is where hosts differ most

`meta` sits after the title (an artist, an album, a version badge), `status` closes that group (a
failure), and `children` sits right of the transport, before the close. All optional: a host with
none of them gets a bar that reads correctly with a title alone.

## Edge slots, and two that replace the bar's own transport

`lead` stands at the bar's left edge, outside its centred part: k7's project puts its way home and its
name there. A host whose name is in `lead` passes an empty `title`, and the title block goes; the system
still reads `title` when the session is on.

`trail` mirrors it at the right edge, and `end` follows `trail`: k7's project puts its pages in the first
and its engine in the second. **On a phone a bar that is about what plays hides the `end` run behind a chevron**
(`usePhone`, its label `more`): the row has room for the transport and the scrubber, and the host's
controls are a tap away above it, in a popover — the one place a bar cannot steal width from what plays.
The STANDBY bar never collapses: it is the host's control row while nothing plays, and that row has the
width for them. Two slots rather than one, because what plays and where to go are two
questions, and a host answering both in one row reads as one list. `end` never shrinks: its controls are
icons with no smaller size, so shrinking the box pushes the last of them off the row — the CENTRED part
gives up its width instead, which is why a phone still shows the record button. `end` draws NO rule of its own: the
engine and its neighbours are a run of controls, and a divider beside an icon reads as an accident.

Below 640 px the gaps between the slots halve: on a phone the bar holds finger-sized buttons edge to
edge, and 12 px gaps are the width of a whole target. The lead keeps its width there too: on a wide screen it
holds a name that may shrink, on a phone only buttons, which would otherwise slide under the
transport. A host whose lead turns to buttons at its own, wider step says so with `leadFixed`: k7's shell
folds at 820 px, and a tablet between the two steps would otherwise draw the play button over its info
button.


`transport` stands where the play/pause button is, and `track` where the clocks and the scrubber are —
`null` for none at all. They are for a host whose transport is more than a range: k7's project has play,
stop, loop and record, and its "where" is bars and lanes, which it draws as a clock and a minimap; a host
that plays a file draws its waveform, and leaves `track` unset for a file it has no shape of, which gives
the scrubber back. The row
around them — the title, the host's controls, the close — stays the bar's, so the shape still reads as
the same player. A host that passes them still passes `playing` and `onToggle`, which the system's
controls use.

## The play button's menu

`playMenu` puts the host's items behind the play button (`src/PlayButton.tsx`): a right-click, the
keyboard's menu key or a finger held still for half a second opens them, and a plain press only
plays or pauses. The play button is where the eye already is for what plays, and the row has no
room for one more button on a phone. The menu is anchored on an inert span over the button rather
than on the button itself, since a menu trigger opens on every press; the click that ends a long
press is swallowed, or lifting the finger would also toggle the transport.

`fill` replaces both the title block and the position row with one node, filling the row right of
the play button: a file's waveform, with the title and the clocks laid over it. Beside a
fill the play button draws no background of its own, since the fill is the surface and a filled
button would be a second one.

`mediaSession={false}` leaves the system's session alone, for a bar whose sound is not this page's: two
bars on one screen must not fight over the lock screen. `onClose` is optional, and a bar without it has
no close.

## The scrubber is an `<input type="range">`

Not a div with a click handler. Dragging, the keyboard and the accessible name all come with it,
and a progress bar that cannot be scrubbed is one people try to scrub anyway. It is `disabled`
until a duration exists, since a range with no maximum is a control that lies about where you are.

**It seeks once, when it is let go** (`useScrub.ts`). A range fires `input` on every pixel of a
drag, and a host that seeks on each sends a burst of seeks — through the engine, say — that the far
end works through long after the finger has stopped. So while it moves the
bar shows the thumb's position locally, clock included, and `onSeek` is called on the release:
the pointer let go (anywhere, not only over the range), the native `change`, a key released, or
focus leaving. Each is the same idempotent commit, so a keyboard's arrows still seek, one step per
press.

**Its hit area is taller than its track.** The visible track is 4 px, drawn behind a range 20 px
tall — 32 px on a device without hover — so a finger finds it without the bar growing. The close
button grows to 40 px on such a device too, centred at the right with room made for it, while a
mouse keeps the small corner button.

**The track is always drawn**, the part not yet played included: a bar whose unplayed half is the
card's own colour reads as no bar at all until something plays. Played is the foreground; the rest
is the muted text colour, faded, which stays visible on both themes. Both come from the design
system's `--color-*` tokens, which are whole colours: a token wrapped in `hsl()` is invalid and draws
nothing.

**The bar's content is centred, at most 800 px wide**: the play button, the title, the position row
and the host's own controls. Stretched across a wide window they end up at opposite edges, the
clocks far from the bar they read, and a long drag is a longer one for nothing. The bar itself — its
background — still spans the window, and the close stays in its corner, where a bar's close is looked
for. It draws no edge of its own: `bg-card` against the page's `bg-background` is what separates the
two, and a `border-t` that names no colour draws in `currentColor`, near white.

## The dock

A host whose pages each mount a bar of their own, and which wants ONE row at its foot whatever the
page, wraps them in `MiniPlayerDock` and draws `MiniPlayerDockOutlet` where the row goes
(`src/dock.tsx`). A bar mounted anywhere inside is drawn in the outlet, through a portal, and COMPOSES the
dock's `end` after its own — so a page's control stands before the host's engine
and take's dot, and no bar has to know the host's slots. A `standby` bar fills the outlet while no other is mounted:
the row never comes and goes, which is the point of a dock. A page that left room under its column
for a fixed bar asks `useMiniPlayerDocked()` and leaves none.

## The close is the only thing that takes the bar away

The component never hides itself: the host decides whether there is a bar at all, and a host with no
`onClose` keeps it for as long as it has something to drive. That is what
lets "what I have just heard" outlive the last sample — a track running out leaves the row up,
saying so through `status`, with the play button ready to start it again.

## The bar is also the system's media session

A phone shows what plays on its lock screen and in its notification shade, and a headset's
buttons pause it: that is what makes an app a media player to the OS, and the browser's Media
Session API is how a page becomes one. The bar fills it from its own props (`mediaSession.ts`),
so every host is a media player without doing anything — and the system's buttons call the same
`onToggle`, `onSeek` and `onClose` the bar's do, so the two can never drive different transports.

- **Two props the bar does not draw**: `artist` and `album` as plain text, since the system
  cannot show `meta`, which is a node; and `onNext`, the system's skip button, absent when the
  host has nothing to skip to.
- **The playhead is reported on a seek, a pause or a new length, not every frame.** The system
  extrapolates a playing position itself, and a host updating `position` sixty times a second
  would otherwise send sixty reports — across a bridge, on the Android app.
- **The session is cleared when the bar unmounts**, since a lock-screen control that outlives the
  player drives nothing.

The browser shows these controls only for a page that is itself making sound — a reference
track playing in the tab. Playing on the engine, the page is silent, and
only the Android app, which relays the session itself, shows them then
(`apps/k7-daw-engine/k7-runtime/docs/android.md`).
