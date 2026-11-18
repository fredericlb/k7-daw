# The recorder's shared pieces

`@studio/recorder` holds what the editor and its hosts share around a take and a clock, each
behind a subpath of its own so a page takes one without the others: `format` (dates and clocks),
`liveValue` (a value that moves every frame), `metronome` (the click) and `webmDuration` (the WebM
header fix). It has no root export and no stylesheet.

## The duration is written into the WebM header, and only the header is read to do it

`MediaRecorder` omits a WebM's duration, which makes the file unseekable: `duration` is `Infinity`
and assignments to `currentTime` are ignored. The editor's camera take is such a file
(`apps/k7/frontend/docs/picture.md`). `src/webmDuration.ts` reads the first 64 kB, adds a `Duration`
to the Segment's `Info` (moving a sized Segment's size and any `SeekHead` position past `Info` with
it), and hands back a Blob of the patched head and a SLICE of the rest — the take's bytes are never
copied into the JS heap. A layout the patch does not know falls back to `fix-webm-duration`, which
reads the whole file and rebuilds it — two to three times the take in memory — so only under 32 MB;
past that the take is uploaded unfixed rather than risk the tab.

## A value that moves every frame wakes only its readers

`createLiveValue` and `useLiveValue` (`src/liveValue.ts`) hold a value set many times a second — the
home page's engine, the roll's hover — and re-render only the few leaves that read it, through
`useSyncExternalStore`. A React state set at that rate would re-render the whole tree under it.

## Every date is worded in one place

`src/format.ts`'s `formatDate` is how the screens print a moment in time.

- **The locale is an argument, never the browser's.** The browser's language is not the app's: a
  French app on an English browser would print English dates beside French sentences. Each caller
  passes the language its catalogue is in.
- **A few named styles, not options per screen.** `day` titles a day, `date` sits in a sentence,
  `dateTime` says when something was made, `time` and `timeSeconds` are a timeline's clocks. A
  screen that rolls its own options prints the same thing one way on one page and another way on
  the next.

## The click is shared

`useMetronome` (`@studio/recorder/metronome`) is a click on the Web Audio clock, not `setInterval`:
each wake-up queues the next window, so a busy main thread delays the scheduling and never the
click.
