# The shell (`App.tsx` and `main.tsx`)

## What wraps the editor (`main.tsx`)

The gates, outermost first: **AuthGate** (the page must hold the backend's token),
**EngineGate** (find this device's own engine, without asking: the one on this machine among
what the registry lists, reached directly on this machine or registered by this device's own account — an
unowned engine is never taken, and none is offered), and **ProjectGate** (rendered by EngineGate: resolve the workspace this editor was opened on into a folder, then mount the editor).
Several tabs may open one project at once; there is no single-tab lease.

**The project opens BEFORE its engine is found, and without one at all.** The files are the
server's file hub's, which answers with no engine bound, so EngineGate opens the project on the
hub's own ticket and looks for an engine beside it. Someone who has never heard of an engine must
still see their song: a full-screen wait in front of it read as "k7 does not work". The editor's
link asks for its URL on every dial and waits for the search's answer, so the engine found by the
search is reached without a reconnect delay; an engine found later — the bar's menu looks again when it
opens — is joined without a remount, unless its files are not the hub's. Once an engine is held, the files'
ticket names it, and that is what binds it at the hub (`apps/k7-daw-backend/docs/files/hub.md`).

**An owned engine that did not answer is looked for again**, three times, five seconds apart. The
probe gives an address 3 s to answer `whoami`, and a page still loading (a slow phone, or an emulator
drawing on the CPU) can hold the reply past that while the engine answered in milliseconds; the
registry naming an engine of this account is the evidence that one is there. An unowned engine is
never retried, as it is never taken. A retry lifts the two-minute rest a loopback that missed is given
(`retryLoopback` in `@studio/engine-browser/probe`), since that address is the one it looks for. When none
is found, the gate says on the console what it was offered.

**Every wait is one bar along the bottom** (`@studio/project-shell/progress`), never a screen: the
sign-in check, the files, the address resolved, the sync, then — with the project already drawn —
the engine looked for, the engine's plugin scan when the project names an external plugin, the
copy, the apply and the warm-up (`components/StartupBar.tsx`). A wait
that hides the project tells a beginner nothing about what they are waiting FOR; a line under a
song they can already read and edit does.

**Play waits for the same bar.** An engine whose socket is up does not yet hold the project: its
files may still be on their way, and the edit it builds when they land drops a transport already
rolling. So Play stays "loading the project" until the file service's `engineLoad` is null, read on
every `/workspace` poll (faster while it is not), rather than freeing on the engine's first frame.
Before the first answer the phase is unknown, and unknown holds Play too: started at null, a Play
pressed in that window reached the engine ahead of the project, and the build dropped it. Every
poll answer is applied, not only a changed one: the answer to the request an engine's arrival
makes can land after a newer one, and a poll that skipped "unchanged" would leave it standing.

**The plugin scan is one more step toasted by the shell, at the top and centred, and only for some projects.** A project whose
modules name an external plugin is held at the file service while the engine scans
(`engineLoad: "scanning"`, `apps/k7-daw-backend/docs/file-engine.md`), and the toast reads "Scanning
plugins… N / M" from the poll's `engineScan`. The project is drawn meanwhile, as for every other
wait; a plugin the stale scan does not know yet shows missing on its node until the scan ends and
the revision moves. A scan the service gave up on is said once, as an error:
the plugins it had not reached stay drawn missing, which is otherwise indistinguishable from a
plugin that is not installed.

**ProjectGate picks nothing.** The home page links here with an address —
`/@local/<workspace>/<project>`, k7 answering the origin root — and arriving without a project in
it draws the home page instead. That address is the router's `basename`, so the app's
own routes are written below it and cannot rewrite it away. The gate is `@studio/project-shell`'s,
and `components/ProjectGate.tsx` supplies this app's wording
alone. Read
[apps/k7-daw-frontend/packages/project-shell/docs/launch.md](../packages/project-shell/docs/launch.md)
before touching any of it, in particular why the address is a path and the workspace is named
rather than spelled out.

## Going into a project, and out of it, loads no page

The router is mounted by the address's **scope** (`src/address.ts`): the home pages are one, each
project is one — keyed by its basename — and the surfaces menu
is one. `Root` in `appMain.tsx` keys the `BrowserRouter` by that scope, so moving inside one
re-renders nothing above it, and crossing to another swaps the whole tree under the router, with the
same teardown a page load gave — the lease, the socket, the editor — and none of its cost: the code,
the sign-in and `AuthGate` stay. The project is still the basename, so everything said above about
it holds inside a project.

What a page load used to forget is now forgotten by hand, and that is the hazard to keep in mind
when adding state: **module-level state kept for ONE project** survives into the next, and most of it
is keyed by a path, which names a file in every project. Each such module registers its forget with
`onLeave` (`src/scopeLeave.ts`), and `Root` runs them all in a render of its own between unmounting
one scope and mounting the next: the file transport's shared reads (`resetFileApi`), the revision
poller's token, autosave's held buffers, the waveform envelopes, the LCD sounds' project, and the
query cache. A module holding per-project state with no `onLeave` hands project A's answer to
project B.

The crossings go through `go` (`src/appNav.ts`): an app address is pushed and drawn in place, any
other is loaded. A same-origin link to an app address is caught at the document, since a router only
writes below its own basename and the home page's rows are plain links — ctrl-click and the rest
stay the browser's. The gate's own departures (Quit, an address naming no project) come through
`setAddressNavigator`. Leaving flushes the autosaves first and asks only when a pane still holds
work that is not on disk (`hasUnsavedWork`), and a Back that would leave such work is put back if
refused; a real reload keeps the browser's `beforeunload` prompt.

### When somebody else takes the engine

**The claim waits for the address.** EngineGate holds its lease only once the gate reports a phase
`projectResolved` accepts (`@studio/project-shell/gate`). A claim is an OPEN, which displaces
whoever held that project or that engine (`apps/k7-daw-backend/docs/engine/claim.md`): made for an
address that names nothing, it threw a real project off the machine for nothing, and the "taken"
answer that came back hid the not-found screen the address deserved.

**Losing the claim keeps the project on screen**, under a banner (`EngineTakenBanner.tsx`), rather
than replacing it. Two tabs or two people on one machine otherwise could not even read their
timeline while the other one played. What stops is what needs the engine: the link hangs up and
its URL answers null (`LocalEngine.detached`, `localEngine.ts`), so transport and live parameters are dead and the
bar reads "no engine". It has to hang up itself: the other project may be in this account's own
engine session, and a socket left open would drive it.

**The files stay editable.** That is the state of a project with no engine at all, which the editor
already supports: the files are the file service's, and its paths are workspace-relative, so a write
lands in this project's files whichever project the engine now holds. What the engine plays from
then on is the other tab's project; this tab's writes reach the engine once it is taken back.

**Taking it back IS opening the project again**: the gate resolves anew (so `/open` runs) and the
claim that follows carries no secret, which displaces the other tab — which gets the same banner.
It is a press, never a timer: two tabs re-claiming on their own flicker the project between them
for ever. Resuming from the idle card is the same open. On an engine that is not ours (`refused`) there is
nothing to take back, and the banner offers nothing.

**The banner floats** over the editor rather than pushing it down: wrapping the gate in a new
element only while taken would move it in the tree, remount it, and re-run `/open` — a take-back
nobody asked for.

`LocaleProvider` sits above all of them: switching language remounts its children, so nothing that
renders a message may live above it.

`BrowserRouter` sits *outside* the gates. The URL names a **screen of the editor**, and the gates
are conditions for reaching it rather than places of their own — so a deep link like
`/timeline?panel=logs` survives them.

### React.StrictMode is deliberately NOT used

In dev it mounts, unmounts and remounts every component, and the imperative WebGL renderer here —
the ui-eyes video compositor — creates and tears down a context per mount. On Firefox that thrash
loses the context and freezes the tab for seconds. An async-initialised WebGL renderer cannot be
made double-mount-safe cheaply, so the dev check is forgone: the chains graph is DOM and
double-mounts harmlessly, the compositor is the one that pays.

### …and the Profiler is behind a flag, for a related reason

`main.tsx` can wrap the tree in a `<Profiler>` feeding `freezeWatchdog`'s `reportCommit`, so a
stall report says which COMMIT it was and not merely how long it lasted. Only commits over the
watchdog's threshold are kept — but the instrumentation bills **every** commit in the subtree to
find them, and this tree commits on meter frames. So: `?profile=1`, or `studio.profile` in
localStorage so it survives the reloads an investigation takes. `installFreezeWatchdog()` stays
unconditional — the heartbeat and the `longtask` observer are cheap and are what turn "it froze"
into evidence at all.

The per-pane render counter in `PaneBoundary.tsx` is behind it too, for the same reason
("The panes are memoised" below).

The flag is `apps/k7-daw-frontend/packages/project-shell/src/profiling.ts`, and it is deliberately ONE flag rather than one per instrument.
The viewport's per-lane drift readout is behind the same one (`picture.md`): it polls and
re-renders its pane at 5 Hz, the same trade — worth it while somebody is looking, and an
instrument measuring itself the rest of the time. Anything else that costs per-frame to *report*
rather than to draw belongs behind it too.

### On Firefox two of the three instruments see nothing

**Firefox implements no Long Tasks API**, so `observe({entryTypes: ["longtask"]})` is a silent no-op
there and an empty ring reads exactly like a quiet thread; the Profiler being off by default empties the
commit ring for a second, unrelated reason. Only `beginActivity()` breadcrumbs really report — and a stall
report concluding *"nothing on the main thread"* from that silence tells the reader to stop looking in
the app. So the watchdog **asks whether it can see** (`PerformanceObserver.supportedEntryTypes`) and says so
once at startup when it cannot.

It has to be TOLD about the second one: an empty commit ring reads identically whether the Profiler
is off or every commit was fast, and the browser cannot answer that — only the app knows. So
`installFreezeWatchdog({ commits: PROFILING })`, and every report then names the instruments that
were **blind** for it, whether or not it found anything: silence from a channel that was switched
off is not evidence, and a tally line can otherwise look like a whole answer when half the report
was never listening.

Given a stall on Firefox: turn the Profiler on (`?profile=1`), since commits DO work there and will name
one if React is the cause; if that stays quiet, reproduce in a Chromium browser, where the long-task
observer attributes the block.

### The fourth line, which speaks when the other three cannot: `tally()`

The three rings above all answer *"what was in flight, and long"*, and that question goes blind two
ways at once. A ring of 64 spans **rotates inside a three-second stall**, so the work that caused it
can be gone by the time the report is written. And a thread saturated by four hundred short tasks has
**no single span long enough to name** — the thresholds that keep the instrument cheap are exactly what
hide it. Both produce the same empty report as a stall that had nothing to do with the app.

`tally(name, ms, bytes?)` adds work up instead of timing it individually: one map lookup per call, read
and **reset on every heartbeat** so a tally never outlives its window and starts measuring the session.
It is always on — it needs neither the Profiler nor a long-task observer — so on Firefox it is the one
line that reports at all.

Two transports report into it, and between them they cover the JSON this thread parses: the file
service (`setFileApiTally`, per VERB and ROUTE — a path carrying a file name would make every read
its own line and the busiest route unfindable) and the engine socket. A stall naming neither is a
stall that is not about data arriving, which is worth knowing in one line.

`fget` is where the second one earns its keep: it collapses a burst of readers into ONE request and
then parses **per caller**, deliberately, since two callers handed one object could corrupt each
other's. The price is real — four readers of a large document parse it four times, on this thread —
and `GET /hints x4` is what says so.

### What NONE of it can see: the cycle collector

Every instrument on this page measures **script**. The heartbeat is starved just as thoroughly by
anything else the main thread does between tasks — so a stall from outside JS arrives as a large
overshoot with every ring empty and every tally at zero. That is not a quiet thread; it is a thread
busy doing something no JS instrument is holding a stopwatch on.

**The measured case, and the one to suspect first when a stall is Firefox-only.** A 9 s profile of
this app sitting on a timeline: 58 % of the main thread in GC/CC, **2 509 of 4 755 samples with the
cycle collector on the stack**, and a single `CCSlice` of **3 804 ms** — the freeze, arriving after
fifteen ~50 ms slices back to back. The self time is all CC graph building (`EntrySlot<PtrInfo*>`,
`CCGraphBuilder::AddNode`, `EdgePool`, `TraversalTracer::onEdge`).

**Chrome cannot reproduce it, and that is not reassurance.** The cycle collector is Gecko's, for
reference cycles spanning XPCOM/DOM C++ and JS; Chrome has no equivalent pass. So "Chrome is fine"
is evidence *for* this reading, and never evidence that the app allocates reasonably.

What feeds it is the size of what is LIVE, and churn on top — objects made and dropped, especially
ones with both a C++ and a JS half. A DEV build multiplies both hard: `jsxDEV`, `runWithFiberInDEV`
and `validatePropertiesInDevelopment` on the stack mean React's development build. Measure a
production build before reading anything into the app's own code.

**React's DEV build is also a LEAK, and the worst of the three.** It records one `performance.measure`
per component render, with a `detail` object for its DevTools track, and the browser keeps every
entry until somebody clears it — about 140 a second on the editor. Measured after a long dev session:
the tab at **3.5 GB**, one CC visiting **8.9 million** live objects and freeing 28 000, slices up to
131 ms, and the cycle collector at **79 %** of the tab's CPU while the app's own JS was 13 %. So every
studio app's entry calls `clearDevPerfEntries`
(`apps/k7-daw-frontend/packages/ui/src/dev-perf.ts`) at boot, which clears the measures and marks every few
seconds in DEV and does nothing in production. Nothing of the apps reads them: the profiler records
them as markers as they happen, so a recording still shows them.

A same-site tab shares the content process, and so the profile: a home page left open beside an
editor is in it. Its workspace poll stores a list only when the list CHANGED (`Home.tsx`), since a
fresh array every three seconds re-rendered every project row and its menus for nothing.

Graphics is the other thing none of it can see, and two rules of this app exist because of it:

- `apps/k7-daw-frontend/packages/ui-chains/docs/wobble.md` — the cables are drawn on a canvas: writing `d` on an SVG path
  puts the layer on WebRender's blob-image path, and one moving cable re-serialises, re-parses and
  re-uploads the whole group every frame (62 % of the content main thread in paint, measured).
- [timeline.md](timeline.md) — live state is drawn by leaves: a timeline BODY subscribed to the
  20 Hz engine frame re-renders an arrangement's worth of DOM to move one line (36 % in graphics at
  ~154 paints/s, measured).

The tool for all of this is the Firefox Profiler, and the rows to read are the ones outside JS.
`~1 280 MozAfterPaint in 9 s` — 142 paints per second on an idle tab — is the number that says the
graphics reading applies; a `CCSlice` in the marker table is the number that says this one does.

`EngineClient` tallies every JSON frame by type, with its byte length, for two things nothing else
sees: the `JSON.parse` runs **before** the message type is known, so no span could open around
it and a slow parse was attributed to nothing at all; and a 20 Hz frame is far under every threshold
here however much it costs in aggregate. `ws state x600 (4.2MB)` is a diagnosis; four blank rings are
not.

## The layout (`App.tsx`)

`App` is the k7 editor itself: **one page at a time, full width** — the timeline, the graph, the desk,
the frame, the master strip or the editor — under a header, with a floating bar over its foot and ONE
panel floating over its right edge. It is the wiring and the layout; almost nothing else.

### The header, the floating bar, and panels that cover

**Panels float OVER the panes and push nothing.** A pane is laid out once, for the whole width; a
panel opening must not make a graph or a roll re-fit under the person's hands. The panes keep a
bottom margin for the bar instead.

| piece | file | what it is |
|---|---|---|
| header | `shell/ProjectHeader.tsx` | Back, the name over its status, the spinner, the warning chip, the git capsule, the pane switcher (centred) and the dock's Mixer; the right panels' buttons are the rail's (`shell/PanelRail.tsx`); laid out in flow: a `flex-1` left cluster, the switcher, a right cluster of the same basis, so nothing overlaps at any width |
| git capsule | `shell/HeaderGit.tsx` | Save with its count, a menu (Pull, Saves); while a merge waits, one "Resolve N conflicts" button and a banner |
| bar | `shell/FloatingBar.tsx`, `shell/ShellTransport.tsx` | the command bar button, the tool, the transport capsule, the pane's capsule (Lane views or Graph views) |
| panels | `shell/panels.tsx` | the registry: an id, a glyph and a title each, in the kit's `SidePanel` (Pad, Desk) or `Sheet` (Phone) |
| dock | `shell/panels.tsx` (`DockFrame`), `hooks/useEditorDock.ts` | the bottom dock, in the kit's `EditorDock` (Pad, Desk) or `EditorSheet` (Phone); it holds the editors, the Mixer or a track's chain |
| phone menu | `shell/OverflowMenu.tsx` | pane, lane views and the panel tiles in one menu |

**The editors, the Mixer and the per-track chain are horizontal, at the bottom.** They live in the
**dock**, along the bottom of Pad and Desk and a partial-height sheet on a phone: a clip, a piano roll
or a signal path reads left to right, and the dock gives them the width the right column never had.
The right panels (Resources, Playlists, Logs, Tasks & history, Export, Settings) stay vertical lists.
The kit's [editor dock](../packages/ui/docs/editor-dock.md) page has the dock's own rules; these are the shell's:

- **The dock has ONE occupant at a time: Events, Mixer or Chain.** Two horizontal editors stacked at the
  bottom would each get half a height that a signal path or a strip needs whole. The dock's accessible
  name follows its occupant ("Events", "Mixer", "Chain"), so a suite finds each the way it finds a right panel.
- **The Events and Mixer buttons switch the dock and stay pressed while their occupant is shown.** Pressing
  the button of the current occupant closes the dock. Filling the slot opens Events, and opening a
  lane's chain switches the dock to Chain; the clip Events held comes back when the dock returns to it.
- **The dock is not a `panel` value.** It coexists with a right panel, so its open state is its own
  (`useEditorDock`) and `panel`, the one the URL carries, is derived: Logs wins, else the occupant
  (`event`, `mix`, `graph`) while the dock is open. `?panel=mix` and `?panel=graph` open the dock
  on the Mixer and the Chain.
- **A right panel opened beside the dock ends 12 px above it** (`--k7-panel-bottom-open`), and the
  toasts sit above the dock (`--toast-bottom`): the dock never covers what it must not.
- **Expand is `ui expand`** (`ui editor` is the source pane's, key `1`). Escape leaves the expanded
  state before it closes the dock, through the popin ladder and through `ui close` alike.
- **The dock's height is the person's, for the session**, not stored: a height stored across sessions
  would meet another screen.
- **Closing the dock lets go of its slot, except from the Mixer**, which drew none.

Three layouts, said on the root as `data-layout`: `phone` (the folded shell), `pad` (a touch screen
that is not folded) and `desk`. Pad and Desk draw the same chrome; the attribute is for the suites.

**The header shows no engine state.** The engine says what it does in toasts — the project waiting for
it, a lost link with Retry, the waveforms being cached (`shell/engineToasts.tsx`); the header only
carries `data-testid="engine-status"` with its `data-state`, which the suites read. A lit light would
be a second voice for what the toast already says.

**The spinner is the background processes.** It shows only while something runs and opens Tasks &
history. The warning chip counts the project's problems (compile errors, the in-place warnings and the problem lines of Logs) and opens Logs on "problems"
(`logsFilter` in the shell store). A verb's refusal is a command's answer, so it is listed under Commands, not Problems.

**The pane switcher is Arrangement, Graph, Source** (Turntables beside Arrangement on a project that
declares a desk). Mastering is not a pane: it is a section of the Mixer, and `ui mastering`
opens the dock on it.

**Command feedback is a toast** ("copied 1 event(s)"): what a command said is shown once, where the
person looks. The line under the project's name (`StatusText`, `project-status-line`) carries only what
a pane says while it is used — a drag's snapped cell, a menu's hint.

### One colour per meaning, across every view

k7 keeps its own values over the shared palette in `src/index.css`, and every surface that draws a
song reads them, canvases included (through the CSS variable, since a canvas cannot take `var()`):

| meaning | token | why one |
|---|---|---|
| the playhead | `playhead` | the timeline, the roll, the minimap, the takes view, the clip's waveform and the desk drew five different lines; an audition's line stays amber, being another clock |
| mute, solo, arm | `mute`, `solo`, `arm` | the timeline's solo and the graph's were two yellows; a lit pad takes dark type, which reads on all three |
| the meter's zones | `meter-ok`, `meter-hot`, `meter-clip` | the floating bar, the desk and the graph's faders say the same thing about the same level |
| violet as text | `primary-text` | no one violet passes both as a fill under white type and as text on this ground (`primary` is the fill) |
| text below muted | `faint` | it replaces `/50`–`/70` fades of the muted grey, which fell to 2.4:1 |

Takes, slices and cues are a SERIES, coloured by order from `@studio/song-schema`'s
`seriesColors.ts`; a track keeps its category colour, and one the compile has not coloured yet is
that scheme's `other` grey rather than a type hue of its own.

**Surfaces are told apart by fill.** The ground is one step deeper than the shared theme's, the
card a step above it, and a hover a step above that — `accent` and `muted` were the same grey, so a
hovered row looked like a panel. A panel that is a surface (the home page's workspaces, the settings
sections, the libraries card) is its fill with no outline; a hairline is kept where it SEPARATES
something, between a header and its rows.

#### The floating bar, and what a SET drops from it

The transport capsule holds Play, Stop, Loop and Record, the clock (`role="timer"`), the song overview
(`img "Song overview"`, the minimap), the tempo and the metre as buttons, the metronome and the master
volume (`slider "Master volume"`), all in `group "Transport"`. A phone moves the clock, overview, tempo and
volume to a strip above the capsule.

**Record has two meanings, and the long press is the second.** A press arms and records the lanes; a
long press, a right click or ArrowDown opens a one-item menu, "Record the output", for what the project
sends out. A project with no timeline has no lanes to arm, so there Record IS the output's
(`shell/OutputRecordButton.tsx`).

**A SET (no arrangement) drops** Loop, Record, the overview and the tool, having no bars to count and no
lane to press on; a desk alone on screen drops the transport buttons too, since they would drive nothing
visible.

**The tool and the Lane views read the shell store** (`tool`, `rulerSeconds`, `lanesExpanded`,
`automationShown`, `pictureShown`). The timeline still holds its own state, so the shell mirrors what it
reports and what the `ui ruler` / `ui automation` commands flip, until the timeline reads the store.

**The right-end capsule belongs to the current pane.** The arrangement shows the Lane views; the Graph
shows `group "Graph views"` (Zoom fit, Auto layout, Stack, the last pressed while stacked); the editor
and the desk show nothing, not an empty capsule. Each view's buttons act on one pane, so a button that
does nothing where it stands has no place there. The Graph keeps its camera, so its pane hands the
bar the fit through the store (`graphView`); Auto layout and Stack are the shell's own commands.

**The clock shows one timecode, and it is the unit's switch.** Bars and beats on a song, seconds
on a project with a picture; a press flips it, and the rulers follow, since a clock and a ruler
counting in two units make every position a conversion. There is no separate clock button.

### On a phone, one menu holds everywhere there is to go

`shell/OverflowMenu.tsx` (`dialog "Views and panels"`, plain toggle buttons with `aria-pressed`, not `role="menu"`;
focus moves in on open and back to the trigger on close): the PANE segmented control, the
current pane's pills (Lane views, or Graph views on the Graph; none on the editor), the tool, the metronome with its level slider, the PANELS tiles (pressed while open) and Settings; the command editor opens from the Settings panel's Commands button, not from the header. A panel
opens as a full-screen sheet.

### The Mixer is a dock occupant: strips left to right

The Mixer is drawn in the bottom dock (`GraphPanel` given its `strips`, then `MixerStrips.tsx`): one
84 px card per strip side by side, a horizontal scroll when they overflow, and **Master as the row's
last strip on every device**: the sum of the lanes, its peak, its fader and an OUT button. The
mastering knobs open in a section under the row, open by default, and OUT folds them away. A footer
says where each strip comes from and links to the Graph. Its header row carries "Mixer" and the strip
count; expanded, the strips take the height. It is the dock's
occupant and not a page so that the timeline stays on screen beside the faders it is mixing.

A strip is drawn by the Mixer itself, not through the graph canvas: a strip is a fader, a pan, a
meter and M/S over the lane header's own `trackControls`, and a second implementation of a channel
strip inside DomChains would drift from it. The cables stay on the Graph pane.

Which modules are strips is the project's answer, not the package's: `mixerStrips.ts` reads it off
the compiled doc and `useProjectDoc` hands the list up. **Every audio lane has a strip**, and
`track new` writes it with the lane
([apps/k7-daw-frontend/packages/song/docs/trackMixer.md](../packages/song/docs/trackMixer.md)): a module of
plugin type `Mixer` (`trackMixer`, `MIXER_PLUGIN`) last in the lane's run.

A song can still have **none** — one whose lanes all run into buses, or one with no `graph.k7x` at
all. So the button stays **disabled** at zero and says why in its tooltip, and `ui panel mix` opened
anyway says so in the dock. A control that answers a click with nothing is worse than one visibly
unavailable, which is why the Mixer button sits apart from the loop that draws the other panel buttons.

- **A track with no mixer module has no strip.** It is not "at unity" — it is a track with nowhere
  to put a level. It simply is not in the row.
- **Two tracks sharing a strip draw one card.** `mixerModules` de-duplicates, because two cards for
  one value would be two controls fighting.

### ONE panel for an audio clip, recorded or not — and clicking one reaches it

`AudioClipPanel` is where an audio clip is looked at, whichever way you got to it: a take that has
just stopped, `ui review <clip>` on an old one, or a **click on an audio event in the timeline**. One
panel for the three, because the route you reach by clicking a clip has to be the one that can trim
it, mark it and bring it up to level. Everything below `clip` is therefore OPTIONAL — a clip sitting
on a lane has no take, no count-in and no armed track — and only `beatsPerBar` and `tempo` are worth
passing anyway, because the twelve-bar window should be twelve of *this* song's bars.

`WaveformView` draws the same waveform and can act on none of it: it is the editor's own pane, a
different surface with a different job.

`AudioClipPanel` takes the graph slot, and it is the only place a clip is drawn. While a take rolls it
shows the envelope the engine is pushing, **growing**; the moment the recorder closes the file it shows
that file, with its onsets and the button that brings it up to level. `rolling` chooses the picture — it
does not mount the panel, which outlives the take, so closing is the × and not the Stop.

**One panel, and not two.** A live monitor of its own would take the slot and keep it, so after Stop the
monitor would be the whole screen and the review nowhere. Three things follow:

- **The live picture goes through `Waveform`**, like the file's. `useLiveEnvelope` hands over
  `{intervalMs, samples}` — the same shape the `/waveform` route answers with — so there is one
  renderer, one set of markers, one behaviour.
- **One envelope, not one per armed track.** The LOUDEST armed input per frame: a panel is about one
  clip, and stacking every lane makes it a monitor rather than a view of a take. The max is what
  keeps a quiet second input from being invisible.
- **Nothing is editable while it rolls.** There is no file yet, so there is no onset to place and
  nothing to normalise — Stop and the elapsed seconds is all it can honestly offer.

The live x axis is nominal: `LIVE_FRAME_MS` (50) per frame, because the engine's push is ~20 Hz and
not a clock. A growing take is read for its SHAPE; the moment timings matter, the file is there and
its own waveform has replaced this one.

#### The take's start is the SHELL's, and the counter measures THIS take

`takeStart` lives in `App`, taken off the store on the `rolling` **edge**. One source, because two
surfaces draw from it — this panel's counter and the growing clip on the armed lane (`RecordingClip`,
`docs/timeline.md`) — and two captures drift the moment they mount at different times. READ from the
snapshot rather than subscribed to, since it changes once per take and a subscription in the shell
would re-render everything 20 times a second for a number that does not move.

The edge, never once per mount, because the panel outlives its take: a capture that survived one would
measure every take after the first against the *first* one's start, while the position it is compared
to is absolute in the song — a take punched in at bar 15 reads 56 beats the instant it begins.
`useLiveEnvelope` clears its buffer on the same edge.

`takeCounter.ts` counts the pre-roll down, then the take up in bar:beat. It is a pure function and not
arithmetic buried in a component that also draws a canvas, because a stale number in it is invisible.
Both results are **clamped**: a pre-roll can never be longer than the pre-roll, and a take can never
have run for less than nothing, so whatever the engine does with its playhead during a count-in the
worst this shows is a number that stops moving.

**The count-in draws nothing, anywhere.** Nothing is recorded during it, so the panel shows the countdown
alone and the lane's growing block does not exist yet — `recordingFrom` (the transport's start plus the
pre-roll) is the one number both read. Neither may take the TRANSPORT's start instead: a block drawn from
it reaches back over bars the placement landing on Stop does not cover, and a picture opened there sits
however many bars of click and room to the right of where it is in the file.

#### How the tab learns a take happened at all

Nobody types a recording. The ENGINE writes the wav, the file service declares the clip and places it
on the lane, and this tab is told by neither — so the document it is drawing is one clip out of date
and nothing in it knows.

The signal is `lastTake` on `/workspace`, read by the 2 s poll (`useActiveProjectFollow`) beside
`activeProject` — every 200 ms from the moment a take stops until it lands, or for 15 s. Two seconds
is the length of a whole breath between Stop and a picture, and the fast rate is bounded because a
take that never lands (a session serving nothing) must not leave the tab hammering the service. Until
the file's picture has loaded, the panel keeps the take's live envelope on screen: the take as it was
heard is a truer picture of it than a "reading" line. The shell does two things with it: `setWs`, which opens the review panel on the new
clip, and a reload, which puts the clip on its lane. Both have to hang off that read and nothing else
— the transport keys go straight to the engine socket (below), so no reply from the file service comes
back to bump a reload, and a take stopped from the floating bar's Stop button would otherwise be on disk, on
the lane, and off the screen.

The first answer only SEEDS: `lastTake` is the service's memory of the newest take it ever wrote, not
an event, so firing on it would reopen this morning's recording every time a tab loads. `isNewTake` is
that rule, and a pure function for exactly that reason.

#### …and the panel can be asked for, not only awaited

The automatic appearance is a moment that passes: `lastTake` lives in the file service's MEMORY, so every
restart of that service loses it, and the ✓ ends it for good as well. A take would otherwise become
unreachable while its file sits in `clips/`.

The bin beside the ✓ discards the take: `event remove` on its event, which closes the panel too. The
file stays in `clips/`, so the discard is one Ctrl+Z like any other edit; deleting the file would make
a misclick on the bin final, and that is the one button on the panel placed to be pressed in a hurry.

`ui review <clip>` names one and opens the panel on it — a `ui` command because it opens a VIEW and writes
nothing, what the panel then does (`clip process`, `event trim`, `clip onset …`) being the brain's. The
named clip outranks the automatic one, without the rolling guard: asking for a clip by name is a
deliberate act.

The lane is looked up rather than asked for (`trackPlacing` in `rows.ts`), and a clip nothing places opens just
the same, labelled **hors piste**. That is the case it exists for: the waveform, the onsets and the
level are properties of the FILE, and none of them needs a placement. `event add <track> <clip>
<timecode>` is the verb that puts such a take back on a lane.

#### Twelve bars, and the wheel opens it out

A minute of take drawn across a pane is a solid block, while the point of looking at one is where the
attacks are and whether the onsets landed. So the panel shows **twelve bars** by default and the wheel
zooms, out as readily as in. The header says `12/34 mes.` with a **tout** beside it, because a window
showing only the first bars without saying so reads as a clip that had been cut.

Everything moves inside the clip's own `from`/`to`: the panel is **caged to the interval the clip
plays** ([clipEditors.md](clipEditors.md)), so "tout" is the whole interval and what a trim cut away is
simply not on screen. The file is still fetched whole (`whole`), so onsets and every command stay in
file-absolute seconds.

`Waveform` takes a `view` for this: a span in seconds, absent meaning all of it. A **window** rather
than a zoom factor, since what a host wants to say is "these twelve bars" and not "×3.7" — and
everything crossing that boundary stays in **absolute seconds**, so a marker's time, a selection, a
trim preview and every callback mean the same thing zoomed or not. Only the pins are filtered: their
`left` is absolute px, so one outside the window would hang in the panel's chrome.

The zoom is **anchored under the cursor** — the same promise the timeline and the roll make through
`useHZoom` — and a pinch does the same, anchored at the centre of the touches. There is **no pan**
gesture: it would be a second gesture on a surface whose drag already means "select", and zooming out
then in elsewhere reaches every part of the take. `barWindow.ts` holds the maths and its edges,
including a tempo that has not arrived yet — a bar of zero seconds would make every take infinitely
many bars long.

#### Trimming moves the WINDOW, and previews before it does

`event trim <track> <at> [gate-dB]` cuts the head and the tail at a gate. It moves that EVENT's window
(`from`/`to` on the placement) and **never the audio**: a take's head is the tail of a count-in and the
player settling, its foot is a room and a hand leaving a string, and both are things you stop *playing*
rather than things you want destroyed. One line to put back, where a rewritten wav is a restore from
the undo journal.

The button **opens a gate rather than applying one**, because where the silence ends is a judgement and
the waveform is what you judge it on. While the menu is open the part that would be KEPT is drawn as a
selection over the wave, so the slider is read against the take instead of against a number. The
preview calls **the same `trimWindow`** the command runs — a preview that disagreed with the act would
be worse than none — so the only difference is bucket resolution.

The gate is **bounded to the window the clip already has**: a re-trim refines `from`/`to` and never
reaches back into material an earlier trim removed — undo is how that comes back. A take entirely below
the gate changes nothing and says so.

Beside it is **operations**: every chain the project and the library offer, normalising among them
(`clip process <clip> <chain>` over a chain naming the module library's `normalize` module). There is no
`clip normalize` and no `track normalize`, and that is the trade — a treatment that can be followed by
another, landing in a stamped render the clip is repointed at, rather than one hard-coded gain written
over the take. See the verbs crate's `docs/processChain.md` for what it costs: a single gain shared
across a whole track's files.

The envelope reaches the command through `CommandHost.clipEnvelope?()`, optional for the reason
`getAudioDevices?()` is: building it needs the engine, which decodes and downsamples into the `.cache`
sidecar. A host without one refuses the trim by name rather than guessing at a window.

#### Onsets are dragged, not retyped

A detected attack is often a hair off rather than wrong, and refusing it only to add another a few
milliseconds away is two acts for one intention. So a pin can be **dragged**: `Waveform` takes an
`onMarkerMove` and the panel sends `clip onset move`, recorded as ONE correction and therefore
surviving a re-detect at another threshold.

The drag lives on the pin — a DOM button — rather than in the canvas's pointer handling, because grabbing
a pin must not also start a selection underneath and the two would race for the same pointer. It commits
on RELEASE, an onset written on every `pointermove` being a file write per pixel. A press that travels
less than 4px is still a **click**, and a click still refuses the onset — the same threshold the canvas
uses, so a twitch while removing a mark does not re-place it.

### The clip under the caret is REPORTED, not drawn

`ClipView` is drawn in one place, the Events panel, and it answers the CLICK rather than the
caret. A panel that followed the caret would redraw a roll at every step of `hjkl` and drop the clip you
were working on as soon as the caret moved off it.

`editTarget` is no part of that drawing. The caret reports what it is over because the command
conditions read it — `hover_midi_sequence` and `hover_audio_clip` are what let a command know which
clip you mean without you naming it.

## A page is a PANE, and there is one on screen

Every pane is mounted always, and always the *same instance* — including the editor, which is never
the home page. Three pieces of state decide what you see, and none of them mounts or unmounts
anything:

- `full` — the page on screen, never empty: `null` in the state means the project's home page (the
  timeline, else the first pane declared), resolved against the layout before anything reads it.
  There is no split: two panes sharing the window each got too little of it to work in, and a second
  view under the page is the panel's job
- `focus` — which pane gets the keys, which is the page on screen; `ui focus next` and `prev` walk the
  ring (`viewPanes(...).ring`, which the editor is not on)
- `panel` — the panel, independent of both

There is no fourth piece: which pages exist is not state. It is a function of what the project
declares, read fresh wherever it is needed. See [shellRouting.md](shellRouting.md) and
[apps/k7-daw-frontend/packages/song/docs/surfaces.md](../packages/song/docs/surfaces.md).

Because nothing is torn down by a change of page, the state that would be expensive to rebuild — the
open file and its undo history, the timeline caret, the zoom stack, the graph canvas — simply never
moves. This is the one invariant to preserve when adding a pane: mount it once, unconditionally, and
let `full` decide whether it is `hidden`.

### Escape backs out ONE layer, outermost first

`esc` in `src/keys.ts` is an ordered list and the first match wins, which is what makes the key mean
"back out of the thing in front of me" rather than one fixed action. From the outside in: a pending
MIDI learn, an overlay (the topmost one), the open Events item, a held tool, a page other than home,
and last the open panel. The ladder is the one place that decides what Escape means; a surface that
needs Escape for itself says so by being an overlay, not by listening on its own.

The ladder only works if each rung is **exclusive**, and that is the whole reason
[`src/popins.ts`](../src/popins.ts) exists. Several overlays answer Escape themselves with a plain
bubble-phase `window` listener and no `stopPropagation` — the phone's
hamburger, the help page, Radix's dialogs — and the shell's own listener is on `window` too. Both fire
on the one keypress, and since the shell's fallback closes the graph slot, dismissing a dropdown would
otherwise throw away the clip or the take open behind it.

So an overlay **registers** while it is up (`usePopin(open, close)`) and the shell reads the registry as
its `popin` condition. Two things fall out of it, both deliberate:

- **Registering is not the same as being closed by the shell.** An overlay that already closes itself
  keeps doing so; the registration exists to make the shell's binding resolve to `ui popin close` and
  stop there. `closeTopPopin` never pops the entry itself — the owner deregisters through its effect's
  cleanup, so a closer that is refused leaves the stack as it found it.
- **A kit sheet answers for itself.** The phone sheet in the UI kit cannot import the app's registry,
  so it closes on Escape and stops the key there; the overflow menu registers as an overlay.
- **A stack, not a count.** "The current popin" is the last one opened, so a menu opened over a dialog
  is what Escape reaches first.

Three things deliberately do NOT take part. The **pickers** (module, asset — both `TreePicker`) and the
**context menu** listen in the *capture* phase and swallow the key, so the shell never sees it. The
**settings PAGE and the module builder** own the surface they are on: the listener returns early. And
**`AudioClipPanel`** keeps Escape for stopping a rolling take, which is neither a popin nor a slot but a
transport action.

The **docked panels** are deliberately not in that list: a docked panel leaves the panes on
screen, so the keys that drive them have to keep working — Space while you watch a level on the I/O tab
is the whole point of that tab not being a dialog over the thing it meters.

## The right panels: one at a time

The buttons that open a right panel — Resources, Playlists, Logs, Tasks & history, Export, then Settings
— are one vertical toolbar on the screen's right edge, under the header (`shell/PanelRail.tsx`), folded by
default under a chevron at its top: the panes keep the whole width until a person asks for the panels. The header
keeps only the dock's Mixer (no Events button: the dock opens on the event a person opens, so a button
would only repeat that). The right panel, the editor dock and the toasts keep clear of the rail
(`--k7-rail-clear`). Each button toggles a panel floating over the right of the panes (a full-screen sheet
on a phone, whose menu holds the same buttons); pressing the open one closes it. **One panel at a time**:
`rightPanel` (the store) and the `?panel=` route state are two doors onto the same slot, and App keeps
them exclusive.

| id | what it holds |
|---|---|
| `resources` (with `references`) | the project's imported audio and video; the workspace's reference tracks, a segment away |
| `playlists` | the workspace's set lists |
| `mixer` | the strips lined up (`GraphPanel`) |
| `events` | the clip, the live MIDI take, the take just filed, or "no event open" |
| `chain` | one lane's signal path (`TrackModulesPane`), else the caret track's graph |
| `logs` | what happened |
| `tasks` (with `history`) | the task list; the undo journal and the sync history, a segment away |
| `export` | export and publish |
| `settings` | tabs: Editor, I/O, Project, Account |

`references` and `history` have no button of their own: the toolbar's Resources and Tasks & history
stand for the pair, and a segment inside the panel moves between the two.

**The I/O tab lists the INTERFACE's inputs, not the song's** (`components/IoPanel.tsx`): every
input device the engine reports, each with its meter and a tuner on the line below, and beside it
the song's input nodes that listen there — or `unused`. A song declaring one input on a sixteen-channel
interface would otherwise hide the fifteen you are trying to find a signal on. Until the engine has
answered, the rows are the song's declared inputs.

**The History panel LIGHTS, it does not travel** (`components/HistoryPanel.tsx`). Hovering a step
rings what it touched on the graph and the timeline — the live ones, as they are now —
rather than swapping them for the project as it was. A step carries the files it
changed, so the marks are read off PATHS (`historyMarks.ts`): a track file is its lane, a sequence
or clip file every placement of that material, a chain file its chain — or only the node a journal
step's command line names in it. A file that is the whole song's (`project.k7x`) lights nothing,
since lighting everything says nothing. Undone steps stay in the list, dimmed: they are what Ctrl+Y
brings back, until the next edit drops them. A step reads as the undo toast reads it — its command's
sentence and subject, "Ctrl+Z stops at the end of the text" below says why — with the raw line as
the tooltip.

Under the journal, the same panel lists the WORKSPACE's sync history (`syncHistory.ts`): its
checkpoints and latest edits from every device, read from the device's own log, or the relay's for a
relay-synced workspace (`apps/k7-daw-backend/docs/files/history.md`). The two stay apart because they answer different questions:
the journal is this project's undo, local and offline; the sync history is what every replica
agreed on. A row opens on what it changed as plain text before and after, with no diff library, and
"Restore to here" asks first, because a restore reaches every device at once. It is the panel's one
write, and it is an edit like any other, so it is itself in the list.

Panels rather than one set of tabs, because they are different verbs — what you **set**, what you
**play from**, what you **listen against**, what you **send**. Settings are things you set and leave, a
set list is a thing you work from, and a reference is a thing you put on beside what you are doing. Nor is the share a dropdown, which is the wrong shape for a surface that *reports* — an export
takes a minute and says where the file went.

None of them is a **pane**. A pane is what the project DECLARES ([shellRouting.md](shellRouting.md),
`apps/k7-daw-frontend/packages/song/docs/surfaces.md`); none of this is the project's, which is why none of it is in the URL
either.

## The open file follows the disk (`hooks/useDiskSync.ts`)

Every command in k7 rewrites k7x — `node param`, `track gain`, a clip drag, a bypass click. An editor
that read its file once and never again would hold a snapshot of the past the moment you touched
anything, and the next save would write it back over whatever had happened since. Silently. **That is a
data-loss bug, not a cosmetic one.**

| the buffer is | what happens | why |
|---|---|---|
| **clean** | adopt the new text without asking | nothing to lose, and anything else makes the editor lie about a file nobody touched |
| **dirty** | ask, and do nothing until told | two versions of one file exist and only the person knows which matters |

Gated on `/revision`, the same ~2 ms change token the diagnostics poll reads, rather than re-reading the
file on a timer. The token moves for OUR writes **and** for edits made outside the app — another editor — which is exactly the set of events worth reacting to. It is asked once for the whole
app, on a two-second poll; see
`hooks/useRevision.ts`.

The poll alone leaves a window: an autosave due in under a second, and a drag or an undo that
rewrites the same file inside it. So the editor's write names its BASE — the text it last read or
wrote, `saved` — and the file service refuses it with a 409 when the disk has moved
(`apps/k7-daw-backend/docs/file-native.md`). A refusal raises the same banner the poll would
(`useDiskSync`'s `raise`), with the buffer held. "Keep editing" makes the disk's text the new base,
so the next write goes over it, which is what the person chose; "take the disk" drops the buffer.
The project journal flushes every pane before it moves (`HistoryDeps.flush`), so Ctrl+Z never
lands under a pending autosave. A write that lands after the pane moved to another file updates
nothing of the file now shown.

A write in flight is the base until it answers. The revision moves when the disk takes the write,
which can be before the write's answer reaches the page; a poll then reads the new text back while
`saved` still holds the old one, and a buffer undone to that old text looks clean and would be
replaced by the read, the undo lost with no write. So the text being written stands in for `saved`
in that decision (`writing` in `decideDiskSync`).
Likewise a buffer found equal to the disk takes that disk text as its base (`onClean(fresh)`), not
the buffer as it reads when the update applies: an undo landing in between would become the base,
and the autosave would see nothing to write.

### Space is the transport, wherever the last click landed

The key listener draws a line between two groups rather than standing aside for one list of tag names
— `INPUT`, `TEXTAREA`, `BUTTON`, `A`, `SELECT`, `contentEditable`:

- **Typing** — a field, or CodeMirror — owns every key it is given, Space included. That is what makes
  it a text editor, and nothing here may touch it.
- **A focused control** — a button, a link, a select — keeps the keys the platform gives it, except
  **Space** and **Escape**. Space, because "press Space to play" has to hold whatever you clicked last:
  a button reached by pointer keeps focus, and the transport failing because you had just clicked Mute
  is the bug. Escape, because a focused button does nothing with it, and Escape's first job is to close
  the overlay that button is sitting in. **Enter** still activates a focused button, so a control
  reached by Tab is still operable from the keyboard.

`blurOnClick` (`keys.ts`) hands back every *other* single-key binding — `1`, `m`, `s`, `f` — after a
mouse click, which the listener lets a focused control keep.

Space takes the **direct route when it can**. `transport play|stop|toggle|rewind` (`BARE_TRANSPORT` in
`src/ui/commands/transport.ts`) are answered by the shell over the engine WebSocket — the same socket the floating bar's
play button uses. The file service route is a real cost, not a theoretical one: it sends this very
message, but only after an HTTP round trip, after the undo journal has walked the whole project twice to
find that a transport verb changed no file (**62 ms warm, 215 ms cold**, on a real song), and behind
whatever else that single-threaded process is doing — a project compile there is **1.5 s**. End to end
on an idle service one keypress costs **110–145 ms**; over the socket the engine acknowledges in
**13–50 ms**, most of it waiting for the next 20 Hz state frame. The delay belongs to the queue, not to
the transport.

Two cases still fall through to the service, which is why the branch is a fallthrough rather than a
send: **no socket of our own**, and an engine **mid-apply**. The WebSocket path *drops* a transport verb
while the engine is applying a change (`useEngineLink.ts`) — a queue there turns one press into three
plays and a rewind — and dropping is right for a button you can see is disabled and wrong for a key
whose whole promise is that it always works.

Nothing is lost by answering them here: they write no file, so there is nothing to journal and nothing
to reload. `transport metronome` is NOT one of them: it rewrites `settings.k7x`.

**Shift+Space** runs `transport replay` — the playhead returns to where playback last started, and rolls
again. That one IS a local command on the WebSocket path, because the seek half only exists there, and
its memory is the shell's own (`replayAtRef` in `App.tsx`): while the transport is stopped the playhead
sits where a play would begin, and the rising edge of `playing` freezes that value, so a start from a
key, the bar, another tab or the CLI all count. Shift+space arrives as `" "` with `shiftKey`, so
`keyToken` spells the modifier out (`"shift+space"`) where a shifted letter arrives already uppercased.

### Ctrl+Z stops at the end of the text, and Ctrl+S answers

Ctrl+Z in the text pane spends the buffer's own history first. When it is spent, the next press
does NOT reach the project journal: it says so, and only a second press within three seconds does.
Someone pressing Ctrl+Z through their typing must not silently undo a clip they moved on the
timeline a minute earlier. The toast then names the ACTION undone, and an empty journal says
"Nothing to undo" rather than nothing.

A journal entry's label is the command line that made it, which is the CLI's word and an id:
`sequence note velocity keys-seq-1 47 0.7` tells nobody which step to go back to. So the toast and
the History panel read it as the catalogue's `readable` sentence in the app's language, followed by
its subject — the first argument, a lane by its label and a clip by its shown name
(`stepSentence.ts`, `clipNames.ts`): "Set a note's velocity · keys sequence 1". Only the subject is
named, because it is the one argument a person recognises; the values after it are the tooltip's.
The raw line stays the panel row's tooltip, the exact thing that will be reverted. A label naming no
catalogue command with a sentence — `edit <file>`, a shell verb — is shown as written.

Ctrl+S anywhere but the text pane — which flushes its own buffer — answers that k7 saves by
itself, instead of letting the browser offer to save the page.

### Headphones, said when a live input becomes audible

A live rig monitors its input on purpose (`apps/k7-daw-frontend/packages/song/docs/liveProject.md`), so the moment the
input reaches a channel it is heard — and a laptop's own mic and speakers feed back. The scaffold
stays audible; what changes is that it is SAID, in a toast that stays until dismissed, at the two
moments it becomes true: a channel picked on the input's card (`node param <input> @channel …`), or
a project opened whose graph already has a monitored input with a channel (`headphonesHint.ts`).
Once per project per browser: a warning repeated on every open would be read by nobody.

### The editor POPIN

A command that sends the editor to a file (`ui file`, `edit clip`, `edit sequence`) has to do something
when the editor pane is off screen. It opens as a **popin** — over the layout, on a dimmed backdrop,
**without the file tree**, because a command already named the one file it is showing. Fullscreening it
would answer a question nobody asked: you wanted the file, and it would throw away the timeline or the
graph you were about to read it against.

**And it opens only once the file has been READ.** `EditorHandle.open` answers whether it landed and the
popin follows that answer; revealed first, a name that resolves to nothing leaves an editor popped open
over your work with no file in it and no tree beside it to say so — a command that reports success and
does nothing being the one failure this shell says everywhere else it will not have.

What it resolves is in `src/openInEditor.ts`, whose third pass is the one worth knowing: **half the
callers name a PLACEMENT, not a file.** The right-click on an event offers `ui file` with a clip's id —
`audio_14_0@98.5` (`@studio/song/placementId`) — wanting `clips/audio_14_0.k7x`, and that `@98.5` is a
subsequence of no path, so the fuzzy pass scores −1 on every file in the project. The material behind
the id is tried LAST, after the written name has had its say, because `materialRefOf` returns a
material name unchanged and an entity may legitimately carry an `@`.

It is the SAME EditorTab wearing different classes, which the mounting invariant forces and which is also
what makes it good: the open file, its undo history and the CodeMirror instance survive being popped out
and put back. A second editor would be a second undo stack.

Three details it needs to be usable:

- **The popin is the active pane** while it is up (`activePane`), or the pane underneath would keep
  answering the keys.
- **It closes on a click outside, the ×, or Escape.** Escape belongs to the editor as well: in vim mode
  it leaves insert mode, and a key that both leaves insert mode and closes the window you are typing in
  is a trap. The shell's listener stands aside for a text editor (CodeMirror is `contentEditable`), so an
  Escape typed *into* the file never reaches the shell. What reaches it is an Escape pressed from outside
  the text — the roll tab, a toolbar button, the backdrop — where "close this" is the only
  reading left.
- **Any change of layout puts it away.** It is a detour to one file, not a place to be, so it is
  deliberately NOT URL state — a reload lands you in the view, not in the popin.

#### An INSTANCE in the popin shows its module beside it

A node's file names its per-instance fields, and what those override lives one `extends:` away — so when
the popin's file has an `extends:` that resolves to a file, the popin splits: the instance on the left,
the **module on the right, both editable**. The module side is a small self-contained editor
(`ModuleSidePane`, its own buffer and autosave — the main pane is THE editor instance and its undo
history must stay its own), and its writes follow the same fork rule as any save: a shared `_modules`
file becomes a project copy, and the pane moves onto it.

The resolution is `moduleBehind.ts` — the same answer a ctrl-click on the extends value gives (`base:`
then `module:` in `hints.paths`) — read from the BUFFER, so retyping the extends line swaps the pane. A
base with no file to edit (`@engine/…`, the `@meta/…` track bases) resolves to nothing and the popin
stays one pane. Popin only: the full-screen editor has the tree, and a second file there is one click
away rather than a question.

**Each pane carries its own widgets**, and a picture both would carry is drawn ONCE. A slice rack docks
THE SLICER at the bottom — one picture with coloured windows, rather than an inline waveform under every
symbol line: the source waveform with every symbol's window in its own colour, the symbol names in the
text painted to match (`sliceSymbols.ts`), and the gutter ▶s auditioning each line. The windows grow
draggable edges only in the pane whose BUFFER holds the symbols, so in the split the slicer sits under
the MODULE pane alone and the instance side stands down rather than showing the identical picture
read-only. `rackKeyOf` (projectPaths.ts) is what lets one map answer for both spellings of "this file is
a rack": a module's folder name, a node's stem.

#### The RACK BUILDER popin

`module create rack` (web-local — the building is the browser's) opens a 4×4 pad grid over the shell.
Drop audio files on the pads, name each pad (its TAP) and name the kit, and choose where the samples go:
this project's `samples/` (the default — `@project/<kit>/…`) or one of the account's libraries'. CREATE
uploads them there and makes the kit a NODE, `nodes/<kit>.k7x`, of the LIBRARY's
`instruments/k7-rack`, on its `samples` engine — `@k7-core`'s when it has one, else any library's (`rackModuleRef`) —
through `node new <ref>`, so the node lands in the graph, journalled and extending the library module
as it stands, then its taps in one write rather than a reload per pad. The builder writes no sampler
of its own: one per workspace would be one copy of the same `type: rack` per workspace, so a library
holding none is said on the popin before any upload runs. A kit is the node's and a
sample lives in a samples/ folder, never in a module (`apps/k7-daw-frontend/packages/song/docs/racks.md`), so the builder no
longer writes a module at all. Auditioning is a local object-URL, with no server round trip.

The notes are the drum-machine page, pinned in `rackBuilder.ts`: bottom-left is 36, rightward, then the
row above — so the grid READS the way it plays, and the symbols are written in that order, which every
other surface then walks.

**Each filled pad carries a ✕ beside its ▶.** A drop REPLACES a sample, so without one a multi-file drop
landing a cell over could only be undone by starting the rack again. Clearing revokes the pad's object
URL — these are blobs the document holds, and a builder filled and cleared a few times would otherwise
carry every sample it had ever seen — and stops the audition when it is that pad playing, because a sound
outliving the pad it came from reads as a fault.

#### A sequence in the popin has TABS

Text / Roll, and only here. A popin was opened by a command that named this sequence, so "now
show me its notes" is the likely next thing; a fullscreen editor pane is the text editor, and a grid
wants more room than a popin has to give.

Both write to the same buffer, so a note drawn in the roll makes the file dirty exactly as a
keystroke does and one Save writes it whichever way you made the change.

**A sequence opens on the ROLL**: its notes are what the command was about, and the k7x is one tab away.
The tab is chosen per FILE — keyed on the path — so each newly opened one is answered for what it is
rather than inheriting the last one's tab. It is a layout effect, so the swap lands before the paint
instead of flashing the text editor for a frame.

### The TAP CHAIN popin — one pad's own chain of effects

`ui tap chain <node> <tap>`, a pad's **Edit chain…**, opens `TapChainEditor.tsx` on the subchain that
pad feeds (`apps/k7-daw-frontend/packages/song/docs/racks.md`, "A tap with a chain of its own"): the modules in order, each
moved or removed, **Add module…** through the ordinary module picker, and the cards themselves below.

- **Every button is a command** — `node tap chain add|move|remove` — so the popin holds no state of
  its own and re-reads the graph after each, as every pane here does. The first module added makes the
  subchain and routes the tap; the last one removed takes both away (`apps/k7-daw-frontend/packages/song-edit/src/tapChain.ts`).
- **The cards are the lane view's.** `TrackModulesPane` draws any list of nodes when handed one, with
  no lane card, inputs or frame, so a knob here is the same knob, on the same stores, as on the graph.
- **The chain starts at a node called `input`**, cabled into its first card: a pass-through that says
  where the sound arrives from and does nothing else — no controls, no menu, no file. Without it the
  cards float with nothing feeding them, and a reader cannot tell which end the signal enters
  (`moduleRow`'s `lead`, in `apps/k7-daw-frontend/packages/ui-chains/src/graph.ts`).
- **The picker hands back a ref** rather than placing a node (`onRef` on its state): the chain is
  written by the verb, not by `node new` and a splice. This project's own modules leave as `@workspace/…`.
- **Not a modal dialog.** It is a hand-rolled overlay at `z-40`, under the pickers' `z-50`, as the rack
  builder is: a modal dialog makes everything outside it unclickable — the module picker it opens
  included — and closes on the click that reaches it. Escape still backs out one layer (`usePopin`).

### The GRAPH SLOT — what takes the graph's place

The chains slot holds the graph, or one of three other things, until the × puts the graph back. That is
where they have room: the timeline stays above, which is what you read any of them against.

| what | opened by |
|---|---|
| a sequence's note grid | `sequence edit roll <sequence>` |
| a clip's content — the waveform for audio, the piano roll for a sequence | clicking the clip in the timeline |
| the take being recorded, and the take just recorded | a take starting to roll (above) |

**One state (`graphSlot`), not three flags.** They cannot be there together, so a union says so: separate
booleans would have to agree about which wins, and the × would have to work out which one it is closing.
A rolling take **evicts** whatever the slot holds — the one case where something other than a click
opens it, and while a take rolls the take is the thing.

The two rows can both show a piano roll, and they are **not the same one**. The grid draws a sequence
FILE's `notes:` array (`@studio/ui-editor`, editing the text). The clip view draws a
PLACED clip's compiled notes (k7's own `PianoRoll`, with the transport's playhead on it). Same picture,
different subject — one is a file, the other is a placement.

Four decisions worth keeping:

- **It is not route state.** `routes.ts` carries the LAYOUT — which panes, which is fullscreened, which
  panel. Both of these name a FILE, and no file is in the URL: the editor's open one is not either.
- **Whatever opens it makes the slot visible** (`full: null`, `view: global`). A view drawn behind a
  fullscreened timeline is a gesture that reported success and did nothing.
- **`ChainsTab` is HIDDEN beside it, not unmounted.** It holds the MIDI runtime the whole shell reaches
  for (`midi.md`), so unmounting it would take `ui midi learn` away for as long as the slot was busy.
- **A grid writes every edit**, unlike the popin's tabs. That pane has no buffer and no Save button to
  lose an edit from, and a grid edit is one whole gesture, so there is nothing a Save would batch.

The two commands are bare-named rather than `ui …` because they name a sequence and open an editor that
WRITES to it — the same reason `event move` and `transport record` are bare.

#### A CLICK on a clip, which is a press that never moved

The timeline tracks that (`DRAG_SLOP`) and a press that never moved shows the clip. Falling through to
`commitMove` would find a move of zero beats and answer "move cancelled — no change": a refusal, for a
gesture that asked for nothing. Two consequences of putting the click on the gesture rather than on the
clip element:

- **A frozen or generated lane's clip opens too.** Neither can be MOVED — there is no `clips:` entry for
  a move to rewrite — so a handler hung off moving would drop the press before it was tracked. Seeing one
  asks nothing of the disk, and a frozen lane's render is exactly the thing you want to look at.
- **A pending `event move` still wins.** If the command bar is collecting that move, the click answers
  the bar; showing the clip is what happens when nothing was asked.

What it draws is `ClipView`, so audio and MIDI are told apart in one place.

## The minimap is the bar's progress

It is a **second view of the timeline**, drawn in the floating bar where a player keeps its scrubber, for
every project with an arrangement — over the timeline too, where it is the one reading of the whole song
the timeline's scrolled window cannot give.

**Folded, it is a scrubber's height; under the pointer it grows upward** to a row per lane, over the
panes, and folds again when the pointer leaves — a drag holds it open. The bar's own height never moves,
so peeking reflows nothing. A finger has no hover: on a phone it stays folded and a tap still seeks.

**Over the editor popin the whole bar floats**, pinned to the bottom of the window: a screen with no
visible timeline is one where you have lost the song, and seeing where you are is worth nothing if you
cannot start the tape. The popin's bottom edge is pushed up by the bar's MEASURED height. Only the editor
popin: the builder, the docs and the settings are **pages** about something other than this song, and
the builder's overlay holds no engine link at all, so a transport over it would be a row of dead buttons.

The floating bar sits outside the popin's backdrop, so scrubbing it is not the click-outside that
dismisses the popin.

## What a click in the timeline does to the Events panel

Two gestures move the slot. **A clip's click** selects it and nothing more: the caret parks on it,
which is what fills the command conditions (`hover_midi_sequence`). **Its double-click** opens its
content in the Events panel, under the timeline still on screen, and lights that button. A
single click does not open the panel because the panel takes half the window and folds the lanes:
selecting a clip to move or edit it must not change the layout under the pointer. **The bare lane**
moves the caret to where you clicked and lets the event go — a click on nothing clears what was open.
The panel stays up and says nothing is open, since it was asked for; Escape and its close button close it.

The caret mark is `pointer-events-none`, so a press landing on it reads as a press on the bare lane;
clips stop propagation, so they never reach the lane's handler at all.

### Double-clicking a lane's header lays its modules out in a row

The gutter is the row's handle: a click selects it, a double-click focuses it — opening its
`nodes/<name>.k7x`, framing the chains it feeds, and laying **one lane's whole signal path** out side by
side over the graph. `trackModules` walks the same reach the framing uses (`subgraphForTrack`, so a send
out of the lane's own chain is in it) and adds the ORDER a node set does not carry, then `moduleRow` lays
it out **as drawn** (`asDrawn`): every card at the size the graph gives it, so the row is a view of the
same nodes and not a bigger version of them. The framing
still happens underneath, deliberately: closing the panel leaves the graph already framed on the
track it was about.

A picture lane gets the same row: it is a lane node in the graph like a sound lane, its chain ending on
`VIDEO_OUT`. Its row has no *audio in* / *midi in*, since `track input` is a sound lane's verb.

**The cards are GLUED** (`moduleRow`'s `gap: 0`): a lane's path is one strip read left to right, and
two cards touching IS the cable between them. That arrow is still in the row, marked `glued` — the
canvas draws no cable for it, since cables paint over the cards and one between two touching cards
would be a loop drawn backwards across the seam, but the live ripple still walks it, so a send further
on keeps moving. **The cables that remain are the graph's own**: a send that skips a module shows as
the longer cable it is. Nothing is invented between neighbours: a seam with no arrow behind it is the
truth about a lane whose chains do not touch, and it offers no `+`. Cables meet the cards on the
**sides** (`flow: "across"`, which moves where a cable ENDS and nothing else), because a row is read
across and top-and-bottom would send a cable diving under a card to reach the one beside it. The socket
plates stay undrawn, being CSS-pinned to the top and bottom edges: a plug on the wrong edge is worse
than no plug.

**Every card takes the row's whole height** — the tallest card's, the lane's clip list included —
so the row reads as one band and the side cables run straight, and the camera zooms so that height
is the pane's height less 4 px top and bottom, so the cards' borders show (`fitAxis="height"`,
`fitPad={1}`, `fitMargin={4}`, over a box `moduleRow` draws round the cards and nothing else). The row is a few cards you came to work on,
so it is drawn as large as the pane allows; the width runs off the right and the wheel walks along
it.

**The LANE leads its own row** (`moduleRow`'s `track`), without the colour band its card wears on the
graph: the whole row is about that lane, so the band would say nothing the row does not. Its cards draw no top or bottom
border either (`.dgx-node.across`): the row reads as one strip, divided only where two modules meet. The signal starts there and its fader is the
first thing in the path, so a row that began at the first plugin was a path with its head cut off — and
the cable out of the lane, the one that says which lane this is, had nothing to leave from. It is the
same card the graph draws, matched on the node's KIND rather than its name, since `trk:` exists precisely
so a lane and a module may share one — clip list included, as the graph draws it. It stands taller
than the plugin cards, and that costs them nothing: no card is stretched to its height, and the zoom
cap keeps a tall row from shrinking or growing the rest. **What feeds the lane comes before it**
(`moduleRow`'s `inputs`): an input, and whatever stands between it and the lane, since that is where
the signal really starts — and the seam between input and lane takes a `+` like any other, which is
how a preamp goes in. A lane that reaches no module and no input leads nothing: the row stays empty and
the pane says so, a single card with no cable not being a signal path.

**A `+` waits on each seam**, shown while the pointer is on it, and **one stands past the last card**,
always shown (`@studio/ui-chains`'s `rowSeams.ts`, drawn by `ui-chains-react`'s `Seams.tsx`). Both ask
the same "which module?" a cable's `+` asks on the graph, and run the same `node insert`. The tail
splices into the arrow OUT of the last card: before the lane's mixer when the row pinned one right —
the fader closes the path, so a module added "at the end" belongs ahead of it — and otherwise into
whatever that card's chain feeds, offered only when that chain has ONE way out, since a fork does not
say which branch "the end" is. In a chain project a seam whose left card is a module shows a **second button** on hover, lower down
(at 80% of its height) and wearing a branch icon: the picked module is made, then STACKED under that
left card (`node stack`) — the two become a subchain run in the card's place, drawn as one column, and
a branch off a card already in a column adds to it. It sits on the seam rather than under the card
because the seam is where a person already looks to add something, and the card's bottom is its bar. The pick gets **no position**: this row's coordinates are its own, so a
point taken from it would pin the new node somewhere meaningless in the real graph. Handed none,
`node insert` lets the layout decide.

**A card drawn here is the graph's card, so it is handed the graph's callbacks** — the bypass, the
alternative badges, the waveform's subscription, the lane's fader and mute, and the browser a file-valued param's
button opens. A control drawn from the same card definition and wired to nothing is the worst of the
three possible states: a `<select>` that will not open reads as a broken app, where an absent control
reads as a view that does not offer it. What this row deliberately does NOT hand them is the
CANVAS-shaped half — dragging a card (`onMoveNode`, whose coordinates are the row's own and would be
written into `graph.k7x` as pins), and the cable gestures, which have no bare canvas here to drop a
loose end on. Rack pads are lit but not firing: auditioning one needs the host's own laid-out nodes to
find the sound behind the symbol, which is `ChainsTab`'s `triggerRack` and has not been lifted out.

### One frame for everything that takes the slot

`GraphSlotPane` is the shell and the title bar. The occupants supply a body and what to call it, and
nothing else about the chrome — written per occupant they drift, into title bars at three type scales.
**The dock's own Close is the only one**: a view never draws a second ✕ of its own, in a
toolbar column or in a header row. It runs `ui close`, as the header's panel buttons and Escape do. Three ways
to wear the frame, because the occupants genuinely differ:

- **`title` (+ `note`)** — a name and a detail. The plain case.
- **`bar`** — a strip of your own, for a header that carries CONTROLS rather than a label: the clip panel
  counts beats and offers Stop while a take rolls.
- **neither** — no bar. A clip's contents draw their own
  header already; a second row above it would say the same thing twice. A lane's row of modules wears
  none either: its first card IS the lane, so a bar naming it would be that card's name again, and
  the row gets the height back. When there is no row to draw — loading, an error, a lane that reaches
  no module — the pane says so in its body, where the row would be.

The **Mixer** is not an occupant of the slot. It shares the dock with Events, and showing it leaves the
slot alone: Events shows it again.

**Filling the slot opens the Events panel.** A lane's modules, a clip's waveform or piano roll, a
grid, a take review — each is asked for, and a request the user cannot see is a miss, so every road
into the slot (`onOpenClip`, `sequence edit`, `ui review`, a lane header's double-click, a take
starting to roll) goes through `openEvent`, which shows the dock on Events (on Chain for a lane's modules). The page is left as it
was: the clip opens under the timeline it was clicked on. Switching the dock to the Mixer keeps the event, so
opening it again brings it back where it was; the keys and the mode badges act only on an event that is on
screen (`shownSlot`).

## The command bar

It is a modal card under the header, over a scrim, summoned by the bar's button, Ctrl/⌘+Shift+P or a
double tap of Shift; it is not always there. It runs the brain's shared commands — the same ones `k7 cmd`
runs on the CLI — so the UI adds no command vocabulary of its own. The catalogue and the argument
completers live in [`../src/palette.ts`](../src/palette.ts); why it is a card is in
[`commandBar.md`](commandBar.md).

It takes focus on a **double-tap of Shift**: two lone Shift presses within `DOUBLE_SHIFT_MS` with no other
key between them, `e.repeat` filtering out a held Shift's auto-repeat. The gesture cannot toggle back out,
because the shell's key listener ignores anything typed into an input — Escape is the way out, and has to
be.

## What a command RELOADS — the token, not the counter

Every writing command ends in a reload, and a reload that is one number can only say **"everything is
stale"** — whatever the command actually did. Six consumers key an effect off it: the compiled document,
the graph, the project errors, the file tree, the completion vocabulary, the open file.

Measured on `01.danse.k7`, per command, on a service that is **one thread** — so these queue rather than
overlap:

| route | cost | can a stretch point have moved it? |
|---|---|---|
| `/hints` | **65 ms / 108 KB** | no |
| `/module-libraries` | **70 ms / 100 KB** | no |
| `/project-doc` | 81 ms / 65 KB | yes |
| `/graph` | 34 ms / 50 KB | sometimes |
| `/tree` | 27 ms / 35 KB | no, unless a file appeared |
| `/project-errors` | 18 ms | yes |
| `/library-chains` | 12 ms | no |

Four answers to one question follow from that:

- **`quietCommand`** — the gesture says *I will reload nothing*. A mixer knob, a mute, a warp point. It
  passes `want: []` so no graph is built either, and whoever must move re-reads its own picture. Its
  token is adopted QUIETLY (`noteQuietRevision`): the tick compares against it, so it does not turn a
  knob into a wave of re-reads — errors, plugins, tasks, the desk, the mastering graph — and `get`
  never hands it out, so a component that re-renders for another reason does not pick it up either.
  Only when the reply's `revBefore` is the token this tab already held, though: otherwise something
  else landed in between — another tab, the CLI, another replica — and the write is announced like any tick.
  The two that must still follow a knob take `useQuietWrites` as a second trigger: the open file,
  which may be the one written, and the history, where the write is a row. A knob cannot break the
  compile, so the errors waiting for the next real change is the accepted cost.
- **`bypassModule`** — the gesture changes the GRAPH and nothing else. A module's on/off is the
  canvas's to redraw: it asks `want: ["graph"]`, draws the answer, and bumps no reload. The full
  reload was every route above re-asked over the tunnel — the compiled project recompiled — for one
  node turning grey. A lane's own card is the exception and takes `runCommand`: its bypass IS the
  lane's mute, which the mixer and the timeline draw out of `/project-doc`.
- **`refreshLibraries`** — the pane says *ask me when it can have MOVED, not when anything moved*. The two
  library routes are fetched on mount and by the two acts that write there.
- **`Reload`** — the command says **which files it wrote**, and each consumer decides.

### A graph only when one is on screen

`runCommand` asks for the graph (`want: ["graph"]`) only while a view of it is on screen — the chains
pane, the graph or mix panel, a lane's own row (`graphOnScreen`). A delete on the timeline otherwise
paid a full graph rebuild, 120–160 ms measured against the 15–30 ms the edit itself took, for a
picture nobody was looking at. A HIDDEN chains pane does not refetch on a reload either: it keeps
the key it last drew and asks when it is shown again. The armed lane's MIDI device is re-read off the
graph only when the reload could have moved a chain (`movedTheChains`): a lane's events, a clip, a
sequence or a curve wire nothing.

That last one is what the reload key carries. `POST /command` answers with `changed`: the files it wrote,
project-relative, taken straight out of the undo journal's own diff — `recordChange` brackets every
mutation and compares the two images anyway, so naming them is free. `App` holds `{ key, files }`; `key`
is still a counter, because "the same file twice" has to fire twice and a list cannot say that.

**`files: null` means nobody said, and every predicate reads that as *re-ask*.** That is what lets a
project opened, an undo, a relayed change and a poll that noticed something name no files at all, and
what makes a service too old or too broken to answer degrade to a plain counter.

The predicates are [`../src/reloadScope.ts`](../src/reloadScope.ts), pure functions with a test each,
because what they encode is a claim about the SERVICE that is not visible from here:

- **`movedTheTree`** — did a file appear or disappear? Exactly what `/tree` draws, so this one is total: a
  rewrite never moves it.
- **`movedTheVocabulary`** — could `/hints` differ? Yes for everything except a **rewrite** of a clip, a
  sequence or a curve. Those three are gathered by NAME (`projectHints` keys them off their filenames), so
  a note drawn, a stretch point placed or a curve point moved changes nothing the completion source
  carries. Anchored at the project root on purpose — a module may be called `clips`, and
  `modules/audio/clips/module.k7x` must not read as one.
- **`movedTheChains`** — could the WIRING differ? No when only `tracks/`, `clips/`, `sequences/` or
  `curves/` were written. Only the MIDI-in lanes key on it (`chainsKey`): `/graph` itself is not
  arrangement-free, since a track's card lists its clips' extents and an automation label draws its
  curve, so every view of the graph keeps the raw key.
- **`movedTheSurfaces`** — only a file under `surfaces/` is read for `/surfaces`.
- **`movedTheAudio`** — could there be a new waveform to prewarm? A placement, a clip, or any file that is
  not k7x — an upload, a take, a render. A knob, a cable or a mute decodes nothing.

Each consumer takes a counter of its own, `useScopedKey(reload, predicate)`, which moves only when its
predicate says so — an effect's dependency rather than a test inside it, so a skipped reload costs no render.

A mute or solo goes through the quiet path and then reloads with the files it wrote, not with `null`: it
rewrites `graph.k7x` and nothing the surfaces or the prewarm read.

**A graph a command hands back is cached as the answer for the reload key it is FOR** — the next key
for a reloading command, the current one for a bypass, which reloads nothing ([`state.md`](state.md)).
Every view of the graph reads that cache entry, so the reload the command causes is not a fetch.
Without the key, a graph handed back while the pane was hidden would be drawn when it was next shown,
over a reload that came after it. A key that turns out wrong costs a fetch, never a stale picture.

**The limit, stated because it is real:** a clip's `source` also decides whether it is listed as an audio
or a video clip. A command that repointed an *existing* clip from a wav to an mp4 would leave that one
split stale until the next reload that does ask. What is deliberately still unscoped: `/project-doc`, `/graph`,
`/project-errors` and the picture, which nearly any project write can genuinely change, and the open
file's disk check, whose path is workspace-relative where `files` is project-relative.

## A fault stays in its pane

Every pane is mounted at once and hidden rather than unmounted, and in React an uncaught throw
unmounts the ROOT — so one fault in a pane nobody is looking at would take the whole app with it
mid-session. Each pane and panel therefore sits in a `PaneBoundary` ([`../src/PaneBoundary.tsx`](../src/PaneBoundary.tsx)):
it draws a card saying which pane failed and a Retry, with the raw message folded under Details — it
is for a bug report, not for the person reading the card — writes the fault to the Logs panel, and clears itself
on the next reload, since new data is a new chance. One more wraps the whole tree in `appMain.tsx`,
the last line before a white page. `createRoot`'s `onCaughtError` / `onUncaughtError` put every
fault in the console with its component stack, and a promise nobody caught goes to the Logs panel.

**A project page loads no home page.** The routes with no project in the address — the home page
and its sibling pages — are one lazy chunk, `projects/HomeRoutes.tsx`.

**A missing chunk is a deploy.** The open page names hashed files the server no longer has, so the
first failed dynamic import reloads the page once ([`../src/chunkReload.ts`](../src/chunkReload.ts),
on Vite's `vite:preloadError`), which fetches the new shell. A second failure within a minute is not
a deploy and is left to the boundary to say. The backend answers a missing `/assets/*` with a 404
rather than with `index.html`: served as a script, the page would fail on a MIME type and say
nothing useful.

**A failed read keeps what the view had.** A 4xx is the service's answer about the project — it
does not compile, the file is not there — and replaces the view; a 5xx, a timeout or no answer
says nothing about the project and does not ([`../src/readFailure.ts`](../src/readFailure.ts)). The
surfaces list matters most: it decides which panes are MOUNTED, and a blip read as "none" would unmount
the timeline, the viewport, the desk and the master strip, with their state. The DJ
desk applies the same rule to its cue sheet, and writes no cue until it has read the sheet once —
the write is the whole file.

### Engine warnings: marked in place, or a toast

The engine raises warnings in its own prose — an automation lane aimed at a parameter that is not
there, a duplicated clip id it renamed, a file outside the folders it may read, a plugin it could
not create. What a person is told depends on whether the warning breaks the project or only one
part of it, and they are never put in the status line: that line is for what a pane reports while
it is used, and a warning there would replace it for the whole session, alarming and unactionable.
The engine's own text goes to the Logs panel, which counts it as a problem and is where a bug
report reads it.

- **A warning that names a track or a module of the open project is marked IN PLACE.** The
  engine's `subject` is a module's id (a track's id for a track); resolved against the project's
  tracks and modules, it puts a warning icon on that track's header in the timeline and on that
  module's card, whose tooltip is the sentence. No toast: the project works, and the part that is
  off is shown where it lives. Both icons open Logs on its problems, as the header chip does.
- **Everything else is a toast**, once per project for the page's life: no subject, or a subject that
  names nothing in the project — a hung engine, an internal failure, a time signature, a record
  folder. Those are about the whole session, with no card to carry them.
- **The count next to the title adds both**: the compile findings, plus the in-place warnings.
- **The set is the engine's own.** After every apply the engine sends the full standing set and the
  start of an apply clears the previous one, so a warning an edit fixed disappears with its icon.
- **The kinds are recognised by text.** The engine sends no code with a warning, so
  [`../src/engineWarnings.ts`](../src/engineWarnings.ts) matches the stable start of each known text
  and gives it a sentence; one it does not know still shows, generically, with the raw words
  after it, so a new engine warning is never silent.
- **Once per project, for the page's life, for the toasts.** Every apply re-parses the document and
  re-raises the same warnings, and a reconnect re-applies; the toast is keyed by the engine session
  and the raw text, held above the hook so a remount forgets nothing.
- **A background job that FAILED toasts too**, through the host's `onFailure`: a cut the backend
  refused is asked for from the bar's play menu, which closes on the press, so its reason would
  otherwise live in a menu nobody is looking at any more. The menu keeps its line for whoever
  reopens it; the toast is what a person actually sees.

## Live state does not re-render the tree

The transport and the engine's live state arrive over the WebSocket at roughly 20 Hz. Held as React state
on `App`, each frame would re-render the whole editor even when nothing visible changed. They live in an
external store (`engineStore.ts`, and `pluginParamsStore.ts` for plugin parameters) instead, and
components subscribe to just the slice they draw. Pulling a live value into `App` state costs a full
render per frame.

**"The slice they draw" means a LEAF draws it.** Moving the subscription off `App` is only half the rule,
and the half that is easy to satisfy by accident: a subscription held at the top of any component
re-renders everything that component draws. `s.playhead` read in the body of `Minimap`, `PianoRoll` or
`WaveformView` reconciles a row per clip, 128 key rows, and a whole canvas twenty times a second — **off
screen too**, because every pane here is mounted exactly once and merely `hidden`. Each has a playhead
leaf instead, the shape `TimelineTab` uses ([`timeline.md`](timeline.md)).

**The status line is live state too.** It carries only what a pane says while it is used, such as a
drag's snapped cell, restated on every cell, so it is not `App` state: it is the small store in
`src/statusLine.ts`, read by one leaf, `components/StatusText.tsx`. What a command did is a toast, not
this line. It takes the free width under the project's name, and a phone draws it under the header, since
it is the shell's error channel. What
the panes REPORT to `App` — the caret, the timeline's view, what the editor has open — goes through a
setter that keeps the current object when the new one says the same thing (`src/shellEquality.ts`), and
the editor reports an open sequence by PATH, never its text, so a keystroke is not a new shell state.

### The panes are memoised, and App hands them only stable props

`App` holds the shell's own state — the caret, the timeline's view, the bar being filled, the
palette, the fullscreen pane — and every change of it renders `App`. What the panes share (the
reload key, the verbs, the overlays, the view toggles) they read from the shell store instead
([`state.md`](state.md)). The six panes (`TimelineTab`,
`ChainsTab`, `EditorTab`, `SurfaceTab`, `MasteringTab`, `ViewportTab`) stay mounted while hidden, so
each is exported `memo`ised, and a hidden pane renders only when something it is HANDED changes.

That holds only while every prop keeps its identity, so:

- **No closure, literal or element is written inline in a pane's props.** The command a gesture
  runs comes from the store (`state/paneCommands.ts`), the rest is built in
  `hooks/usePaneCallbacks.ts` from refs, setters and stable callbacks; EditorTab's two builders are
  built above `App`'s return and are `null` while closed. `src/paneProps.test.ts` reads `App.tsx` as
  text and fails on an arrow, a literal or an element in a pane's props — nothing here mounts `App`.
- **A callback that reads fast-moving shell state reads it through a ref** (`openInEditorRef`,
  `paletteOpenRef`), the pattern `graphOnScreenRef` already uses, so the fullscreen pane or the
  palette opening does not hand the chains pane a new insert callback.
- **A reload is a real change, and the panes render on it.** `projectDoc`'s identity is the signal
  `App` reads to release the pending edits and reset the metronome override, so it is never kept when
  equal, and the graph's `cardProps` change with it. What is kept when equal is what nothing reads as a
  signal: the track order, the project's tasks (`keepIfSame`, `src/shellEquality.ts`).

`?profile=1` (below) also wraps each pane's `PaneBoundary` in a `<Profiler>`, and
`window.__k7PaneRenders` then holds, per pane, the commits and their render time. A memoised pane
that skipped its render still reports a commit, at a fraction of a millisecond: read the time.

## The catalogue is the app's; the ARRANGEMENT is the person's

`ui settings commands` opens a page where a command can be renamed, given an icon, told which right-clicks
may offer it, and pulled up or down the order menus list them in. What the commands ARE stays the app's — this is a layer over the catalogue,
kept in `commandPrefs.ts` and applied by `orderCommands` / `labelOf`.

- **Keyed by the canonical LINE** (`event trim`, `ui help`), not by the sid. The sid is the more stable id
  but only the backend's commands have one — the shell's own `ui …` lines do not — and a page that could
  arrange half the catalogue would order a menu it does not draw.
- **The entity a command acts on is the first word of its line**, which is what the filter offers — so a
  command added to the app is filterable the day it exists. A PLURAL head folds into its singular
  (`entityGroup`): `events move` lists among the `event` ones, marked with a layers glyph, because acting
  on several at once is the same subject taken several at a time and a group holding one command hides it
  from the only filter anyone would look under. Derived from the catalogue, not tabulated: a plural whose
  singular nobody wrote keeps its own group rather than inventing a subject out of a suffix.
- **A partial order.** Commands someone has placed come first, by their number; everything else follows in
  catalogue order. A total order would mean every command added later arriving at a position a saved list
  had already decided — and this is what lets you pull three commands to the top without numbering the
  other two hundred.
- **Applied once, over the palette's built list.** The palette is the one surface that lists every command,
  so ordering anywhere else would leave the page describing an order nothing obeyed. The label replaces
  the sentence the palette MATCHES on, deliberately: someone who renamed a command should find it by the
  name they gave it.

It lives in the ACCOUNT's preferences, not the rig's — what you call a command follows you to another
browser, where which audio interface to open belongs to the machine. `K7Prefs` widens `EnginePrefs`,
which is the extension point that type documents, and `patchPrefs` merges, so writing this cannot wipe the
engine id beside it.

**And the backend's zod schema is what actually decides**, because a field it does not name costs this
feature its persistence. A zod object strips what it does not declare, so a layer
`backend/src/server.ts`'s `prefsSchema` says nothing about is accepted, answered `{ok:true}` and dropped:
renaming works, saving says it worked, a reload shows the catalogue's own names again, and the disk holds
`{"engineId": "…"}` alone. Widening `K7Prefs` is half of adding a preference. The router refuses a write
it would have to shorten (`droppedKeys`, `apps/k7-daw-backend/packages/k7-front/src/routes/prefs.rs`), so an omission is a 400
naming the field rather than a feature that quietly does not last the session.

### A name is a SENTENCE, so there is one field per language

The row carries one text field per locale the app is built with — `en` and `fr` today, read off
Paraglide's `locales`, so a third language grows a third field the day it is added rather than the day
somebody remembers this screen. One field would mean one string for every language: renaming a command
while reading French would rename what English shows too, with nothing on screen saying so. The split
follows the line the catalogue already draws — `CommandInfo.readable` is `{en, fr}` while the command's
NAME stays the CLI's in every language, because a name is an identifier and a sentence is an explanation.

- **Each placeholder is the catalogue's own sentence IN THAT language**, which is why `PaletteCommand`
  carries `readableBy` — the whole table — beside the `readable` that `buildPaletteCommands` resolved. A
  placeholder has to be what that field would leave standing, or an empty field reads as "nameless"
  instead of "unchanged".
- **No borrowing between languages.** A command named only in French shows the catalogue's English
  sentence in English. Falling back to the other language would put French words in an English menu,
  which is the thing this split exists to stop.
- **`labelIn` answers as AUTHORED, and the trim lives at the two places a name is drawn** (`labelOf`, the
  context menu's row). Trimming in the reader breaks the space bar: the field is a controlled input, so a
  trailing space vanishes as it is typed and a name of more than one word cannot be entered at all.

### …and this page has a Save button, which nothing else in the shell does

The app's rule is autosave, and this page is the second exception after the module builder. The reason is
not taste: **without a button it is unusable to type in**. Three costs, all paid per character — a `PATCH`
of the whole preferences document; a `setState` at the ROOT of the app, since `commandPrefs` lives in
`App` where the menus, the key listener and the palette all read it, so the whole shell re-renders behind
the overlay; and `keyForCommand(canonical, prefs)` once per ROW, each call building the whole effective
map.

So the page holds a **draft** and writes once, on the button: the second cost goes because the draft
lives in the leaf, the third because the effective map is built once per edit and inverted into
`canonical → label`. The rows are `memo`ised on top of that, which works only because `setCommandPref`
copies the document shallowly — every untouched entry keeps its identity, so one row comes back and not
two hundred.

Two things follow from the button: **a shortcut assigned here does not run until it is saved** — the app
reads the SAVED document, the header says `non enregistré` while the two differ, and `annuler` throws the
draft away — and **leaving asks**, since `← retour` confirms while the draft is dirty. That is the only
thing autosave never had to answer for.

### Shortcuts are the fourth field of that layer

Each row carries a key cell: click it, press a key, and the pref stores the raw `keyToken` (`"g"`,
`"shift+space"`) under `key:`. Escape renounces the capture, Backspace removes the shortcut (stored as
`KEY_NONE` when the command has a default — deleting the pref would resurrect it), and choosing the default
again drops the field.

The rules live in `effectiveKeys` (keys.ts), pure and tested: an assignment MOVES a shortcut rather than
doubling it, a taken token is stolen outright and visibly, and the CONDITIONAL bindings — the esc ladder,
`f`'s interpolation, `m`'s three meanings — are not remappable, being rules rather than shortcuts;
`resolveToken` falls through to them only for tokens the effective map does not name. App's listener, the
help legend (`shortcutsByCommand`) and the pane buttons' tooltips all read the same map, so a moved key is
advertised wherever keys are shown. The focused-control rule follows the COMMANDS rather than the tokens:
what comes back from a focused button is whatever runs a `transport …` command, wherever the person has
put it.

### …and the fifth is WHERE the right-click offers it

A multi-select per row (`components/CommandMenuPicker.tsx`), listing the entities of `MENU_ENTITIES`: a
track, an event, a bloc, a réglage, a repère… Several at once, because one verb genuinely belongs on two
menus — `node param` is offered on a block and on the knob itself.

It is **a layer over a catalogue that already has an answer**: `commands.yaml`'s own `menu:` says where a
command belongs, and a command that says nothing is offered wherever its arguments fit
([contextMenu.md](contextMenu.md)). So the cell shows the catalogue's list until somebody picks another,
and `setMenuEntities` writes the shortest spelling that means the pick — nothing at all when it matches
the catalogue, so a catalogue corrected tomorrow still moves the commands nobody arranged by hand.

It replaced a checkbox, which could only take a command off every menu at once. That was the whole
vocabulary available before commands said where they belonged, and it is still the `false` this field
writes when nothing is ticked.

### Context menus draw those icons

`MenuItem` carries a `verb` — the command WITHOUT the entity and untranslated (`node bypass`, not `bypasser
gtr`) — which is exactly what the prefs are keyed by. Kept as a field rather than re-derived from
`command`, so a menu never has to un-localize a line to find out which command it is; and two entries for
one verb on two different tracks are one command wearing one icon. `contextMenu.ts` stays a pure function
of the catalogue and the thing clicked: the person's layer is applied by the RENDERER, because an
arrangement is a matter of who is looking and not of what an entry means. The icon gutter is held whether
or not there is an icon — a half-decorated menu with a ragged left edge reads as a rendering fault.

The icons are **lucide**, already this app's icon set. What this page adds is resolving one BY NAME, which
cannot be tree-shaken: everywhere else imports the icons it draws, while "whatever the person chose" is not
known at build time. `lucideIcons.ts` loads the set with `import("@studio/ui/icons/all")` and caches it.
A menu opened before that lands draws its labels and no icons — the honest degradation, since the label is
what an entry MEANS. A menu none of whose rows carries a picked icon does not ask for the set at all: the
first right-click of someone who never chose one costs nothing.

**The set is reached through its own module, and that is what makes the split real.** `icons/all` is lucide's
per-icon index, a module the static imports never touch, so the bundle puts all ~1850 components in a chunk
of their own, loaded the first time a name has to be resolved, while the static `@studio/ui/icons`
imports tree-shake down to the icons the code draws. Read through the `@studio/ui/icons` namespace
instead, the same lookup kept every icon in the first screen's chunk — about 190 KB gzipped to draw thirty.
Measured on a build: the icon chunk is ~150 KB gzipped, and the home page carries none of it.

## The entry is split by ADDRESS, and the home page is what that is for

`main.tsx` branches on the address before importing anything heavy, and `appMain.tsx` must too: a static
`import App` there makes opening the app at `/` parse and execute the entire editor — CodeMirror, dagre,
`@studio/ui-eyes`, `@studio/ui-turntables` — before drawing a single workspace row. The symptom is a page
that opens and stays blank for seconds, which reads as the app hanging rather than as a bundle being parsed.

The screens that are not the home page are therefore `lazy()`: `App`, `EngineGate`, `LanSurfaceGate`,
and the home's own other pages — the libraries,
the tasks and both settings pages. One address is ever taken, so one of them is ever fetched. `ProjectGate` loads
`App` the same way and starts its download as it mounts, so the project gate draws while
the editor arrives; an `import App` there would make every gate wait for the whole editor.

Inside the editor, what opens on demand loads on demand: the module and assembly builders, the video export
(its muxer is the largest dependency there is), the icon set (above), and the drawers of the DJ desk and of
the viewport, which only a project declaring that surface fetches (`apps/k7-daw-frontend/packages/k7-plugins/docs/plugins.md`).
The code editor is its own chunk too (`components/EditorPane.tsx`): it mounts the first time its pane
is on screen and then stays mounted like every pane, and the chunk is fetched at idle so that first
show rarely waits — a project opened on its timeline no longer builds a CodeMirror view nobody sees.
The playlist and chains builders' YAML panes (`components/LazyYamlPane.tsx`), the help page's
`marked` and the camera take's duration fix (`@studio/recorder/webmDuration`) are fetched where they are first used.
Measured on a production build, in gzip, each page with every chunk it
imports statically:

| what | loads |
|---|---|
| **home page, sign-in** | **~235 KB** |
| opening a project | ~900 KB in all |
| on demand | CodeMirror ~220 KB (shared with the editor), icons ~150 KB, video export ~135 KB, builders ~45 KB, the desk ~25 KB |

What stays eager is what the home page actually draws: the gates, `Home`, and the shared vendor chunks.
The gates and the home page keep the editor's dependencies out of that list by importing the light half
of a module, never a schema, a parser or a package's whole index for one constant:

| the home path imports | rather than | which would bring |
|---|---|---|
| `@studio/ui-editor/autosave-buffers` (`appNav`) | `autosave` | its validator: the song schema, `yaml`, the k7x printer |
| `@studio/song-schema/moduleFiles`, `projectShapes`, `timecode` | `yaml/moduleSchema`, `yaml/projectSchema`, `yaml/placementSchema` | zod and every schema |
| `scalarField.ts`, only from the editor | `projectPaths.ts` carrying it | the k7x parser |
| `rankCommands.ts` (`CommandBar`) | `palette.ts` | the editor's command registry, every command's sentences in both languages |

A schema re-exports what its light half holds, so a caller that already loads the schema need not know.

The first screen's chunk is started as `appMain` evaluates, beside `AuthGate`'s `/auth/me`, rather than
once that answers: on a phone the sign-in check and the download then overlap instead of following each
other. Only the home page and a project's `EngineGate` are started this way, the two a launch opens on —
and a project address starts `App` too, the longest download and parse of any start, which would
otherwise wait for `ProjectGate` to mount behind the sign-in check and the engine gate. Beside them
goes the file hub's hello (`engines.hostedFiles()`), which needs only the cookie: the home page's
workspace list and a project's engine probe both wait on it. `/auth/me` is asked once per start
however many screens ask (`api.getMe` shares a signed-in answer for a minute), and the home page's
workspace list is asked as soon as the hello answers, from its chunk's evaluation rather than from
its first render (`settings/accountFiles.ts`, `prefetchHome`).

**The built page names `appMain`'s chunks.** `main.tsx` reaches `appMain` through a dynamic import —
it is what keeps the docs host off the editor — so the browser would learn of it only once the entry
has run, a third hop after the HTML. `preload/vitePlugin.ts` writes a `modulepreload` for `appMain`
and its static imports into the built `index.html`, and they arrive beside the entry. The docs host
serves the same page, so it downloads that set without running it, the price of one page for both.
`Booting` is the one thing on screen while a split-out screen arrives — the window is already painted in
the browser's own by then, so it only has to say "coming".

## The `ui` namespace

The shell's own commands are **one registry**, `src/ui/commands/`: each entry is a palette row — its name,
its sentence in each language, its arguments — and the handler that runs it, grouped by what they act on.
The palette's catalogue (`LOCAL_COMMANDS`, `LOCAL_READABLE`, `LOCAL_ARGS`) and the `ui` gate's two lists
(`localCommands.ts`) are derived from it, and `App`'s `runLocalCommand` is a lookup in it. A command
described in three places drifts in three ways — listed and not dispatched, dispatched and not listed,
classified on the wrong side of the gate — and one table makes each of those unwritable; the test that
remains (`localCommands.test.ts`) holds the table against itself. A handler reaches App only through an
explicit context (`src/ui/commands/context.ts`), rebuilt every render and read through a ref, so the
dispatcher never changes identity and no handler sees a stale value.

The view commands live under a `ui` prefix, stripped in one place rather than by renaming every entry.
The pairing is **enforced both
ways**: a `ui` command typed bare is refused, and so is a non-`ui` command typed with the prefix. `ui`
promises "touches nothing of the project, does not move the transport", and a `ui locate` that quietly
moved the playhead would be a worse lie than having no namespace at all.

**A server command is never taken for a bare `ui` one.** Stripped of its prefix, a `ui` head can share
its first word with a server verb — `ui curve` and `curve set`, `curve point add`. So the refusal of a
bare `ui` line asks the server's catalogue first (`uiVerdict`): an unprefixed line the catalogue names
goes to the server, past every `ui` handler, which would otherwise take it by that first word. A test
walks every command of `commands.yaml` and holds that none is refused bare, so a new `ui` head cannot
silently swallow a verb again.

It is not "writes no file", and the difference is deliberate: a pin, a language and the vim flag are
remembered — in `localStorage` and in the account's preferences — and remembering how you like to work is
not editing a song. What draws the line is the PROJECT.

**A better subject beats the rule.** `selection copy` and `selection paste` sit beside the catalogue's
`selection consolidate`: the head names what all three act on, the rectangle you drew. The namespace rule
alone would make the copy `ui` and the paste bare, since it writes through `events add`, which puts one
gesture's two halves on either side of the line. The rule is what a command falls back on when nothing
better names it ([clipEditors.md](clipEditors.md)). `ui copy path` is not one of them — that one puts a
*path* on the system clipboard and is a different act.

A line the shell recognises but cannot act on is reported as *handled* rather than passed down: the backend
has no such command either, and "unknown command" would say less.

The tree's verbs are an **`fs` group** — `fs file new`, `fs folder new`, `fs file rename` — sharing a head
with nothing, which is what keeps `isUiLine` two lists and no special case: `ui file <path>` opens a file,
and a bare `file <verb>` doing something *to* one would collide with it on the head alone. They are also
the one family that acts on the workspace's files rather than on the song.

`ui external <surface>` reads an address and puts it on the clipboard, touching neither the project nor the
transport nor even this shell's view. It **does not navigate** — the whole point of the address is to be
carried to a second device, and following it here would replace the editor you are standing at with the
tablet's screen. The line is echoed in the status bar as well as copied, so a browser that refuses the
clipboard still leaves it readable.

## Every action on screen is a command

There is no button, menu entry or toggle in this shell that the command bar cannot reach. That is a rule,
not a coincidence, and it is what makes the palette a real answer to "what do you want done" rather than a
shortcut list for the half of the app somebody remembered to wire. It also means a gesture is describable:
a bug report, a doc page and a key binding can all name the same thing.

### …and the log holds every one of them

The Logs panel is a **transcript of what ran**, not of what was typed. Every command reaches it as a `cmd`
line, whichever way it was triggered — the bar, a key, a button, a right-click, a cable dropped on a knob
— because `runCommand`, `quietCommand` and `bypassModule` are the only doors and all log before they dispatch. The
buffer is the shell's and fills whether the panel is open or not; only the engine machine's merged process log is
polled on demand (`useLogs`).

Two cases are worth saying out loud, because each is a command nobody typed:

- **The picker's first half.** Choosing a module from the canvas menu runs a `module new` ahead of the
  `node new` you were prompted for. Unrecorded, it would leave a transcript showing a node appearing with
  no module ever created.
- **A command aimed elsewhere.** The chains builder's verbs run against a library chain, not the open song,
  so its lines carry the chain's ref: `› [process/normalize] node new x`. Without it the transcript would
  claim an edit to the project that never happened.

What is deliberately NOT in there: anything that goes through a ROUTE rather than a verb — a file write, a
module save, a probe. Those are not commands, and a log that mixed them would stop answering the question
it exists for.

Keeping it costs one habit — **the button runs the command, it does not call the handler**:

| what draws it | the command | where the behaviour lives |
|---|---|---|
| the header's Mixer button, and notes | `ui mixer`, `ui notes` | `toggleMixer` (→ `togglePanel`) / `toggleDocs` in `App` |
| the minimap's stack button, left of zoom fit | `ui stack` | `toggleStack` in `App` |
| the panel toolbar | `ui panel <event\|graph\|mix\|logs>` | `togglePanel` |
| the log panel's *clear* | `ui logs clear` | `useLogs` |
| a pane button in the floating bar (the desk, the timeline…) | `ui <pane>` | `fullscreen` / `setFull` in `App`, like any pane |
| the timeline header's clock / height / automation | `ui ruler`, `ui lanes`, `ui automation` | `TimelineHandle` |
| the per-track view's curve button, in its left column (`ModeStrip`, the timeline's) | `ui curves` — shows or hides its curve cards | `App` |
| the per-track view's *audio in* / *midi in*, above it — greyed while something already feeds the lane (`laneHasInput`) | `track input <track> audio\|midi` | `App` |
| the timeline corner's ＋ piste | `track new`, and `picture lane new` on a project with a viewport | `App` — the menu appears only when there are two kinds of lane, see [timeline.md](timeline.md) |
| a timeline header dragged to another row | `track order`, `picture lane order` | `App` — the drag names the lane whose place is taken, see [timeline.md](timeline.md) |
| a layer moved or resized in the VIEWPORT | `picture rect` | `App` — the selection box is `components/ViewportOverlay.tsx`, see [picture.md](picture.md) |
| a picture lane's **A** button, and `r` on its row | `ui picture arm <lane>` | `App` — browser-side like `ui midi learn`, since the camera is the browser's ([picture.md](picture.md)) |
| the cog | `ui settings` — toggles its panel | `App` |
| the header's Playlists button | `ui playlists` — toggles its panel | `App` |
| the header's Export button | `ui share` — toggles its panel | `App` |
| the settings panel's tabs | `ui settings editor\|io\|account` | `App` |
| the settings panel's *Commandes* | `ui settings commands` — the one that is still a page | `App` |
| the account tab's rows | `ui engine`, `ui workspace quit`, `ui language` | `App` |
| the share panel's rows | `project export` | the verbs crate (`apps/k7-daw-wasm/verbs`) and `hooks/useShare.ts` |
| `ui external <surface>` | the palette's way to a surface's address | `App` |
| the editor tab's one toggle | `ui vim` | `toggleVim` in `App` |
| Ctrl+Z / Ctrl+Y | `project undo`, `project redo` | `@studio/project-shell/undo` |
| the file tree's right-click menu | `fs file new`, `fs folder new`, `fs file rename`, `fs file move`, `fs file delete`, `fs file upload`, `ui pin`, `ui copy path` | `EditorHandle`, and `FileTreeHandle` under it |
| the clip panel's *stop*, while a take rolls | `transport stop` | the engine |
| the roll's *×2* | `sequence double <séquence>` | the verbs crate (`apps/k7-daw-wasm/verbs`), over `doubleSequence` — see [clipEditors.md](clipEditors.md) |
| a `.dawproject` / `.als` clicked in the tree | `project import <path>` | `useProjectImport` |
| a file dropped on a timeline lane | `event add`, `picture add` — the PLACING half; the upload and the `clips/<id>.k7x` go through routes | `hooks/useTimelineDrop.ts`, see [timeline.md](timeline.md) |
| the master meter's *fix* | `project master fix` | `fixClipping` in `App`, over `MasterMeterHandle` |
| the channel strip's mono/stereo pairing | `system channel type <ch> <mono\|stereo>` | the verbs crate (`apps/k7-daw-wasm/verbs`) — the rig's file AND the engine |
| the Playlists panel's *＋* | `playlist new <name>` | the verbs crate (`apps/k7-daw-wasm/verbs`), in `_playlists/` |
| the Playlists panel's *Separate the stems* | `playlist stems <playlist>` — the bar only starts it | the verbs crate (`apps/k7-daw-wasm/verbs`), through the shell, see [playlists.md](playlists.md) |
| the settings page's *Separation chains* | `system stems <chaîne>` — the page itself writes through `PUT /account/cold-chains/stems`, since it has no project to run a command in | the verbs crate (`apps/k7-daw-wasm/verbs`) — `<state>/preferences.k7x`, read by every screen that could start one |

Two of those handles exist only for this: `EditorHandle` carries the tree's own verbs, and `FileTreeHandle`
exposes the two things nothing else holds — the pins, which are that component's `localStorage`, and the OS
file pickers, which are its two hidden inputs.

Three of those rows have TWO answers. `ui select`, `transport locate` and `transport loop` reach whichever
grid is on screen: the clip pane's caret while it is open ([clipEditors.md](clipEditors.md)), the
timeline's otherwise — except that `transport loop` goes to the clip pane only when IT holds the
selection, as Shift+L's conditions already say: the ruler's "Loop selection" is about the timeline's
range, and the clip pane open below it would otherwise take the line and find nothing to loop. The commands are one thing, and which surface answers is a question about what you
are looking at, not about the verb. `ui zoom` stays the timeline's alone: it is the only one that keeps a
zoom stack. `selection copy` has two answers as well and takes them in ORDER — the clip pane, then the
timeline, first answer winning — because it is the only one that asks the pane a question instead of acting
on it, so a pane in front with nothing selected must not end the question.

Three things are deliberately NOT commands, and the line is the same one every time — a command names
something you could *ask for*, and none of these is:

- **Disclosure.** Opening a dropdown, expanding a tree folder, showing the sidebar. Asking for it and doing
  it are the same act; there is nothing left for a command to add.
- **A dialog's own buttons.** Confirm, cancel, "continue editing". They answer a question the shell asked,
  so they cannot be reached from a bar that is not the question.
- **Continuous gestures.** Dragging a clip, growing a selection, panning the graph, the waveform's onset
  sensitivity, the roll's velocity drag. The timeline's `event move` is the command and the roll's is
  `sequence note velocity`; the nudging is the view's, as `keys.ts` says.
- **Typing into the open buffer.** The video readers' *Set From* / *Set To* look like actions and are not:
  they write a number into the file the editor is showing, which autosave then saves like any keystroke.

The two transport buttons in the floating bar are the clearest place a button does NOT dispatch its command, and
the reason is latency: `transport toggle` is a CLI verb, so it travels to the file service and back, while
the button already holds an open socket to the engine.

One of the Playlists panel's gestures takes a road of its own, in [playlists.md](playlists.md):
adding a record from the SEARCH writes into the OPEN BUFFER rather than the file, because a command writing
the file underneath a modified buffer would race the autosave.

## Two-step flows

Some gestures need a second command the user did not type, and both are held as pending state in `App`:

- **The graph's `+`.** Clicking an arrow's `+` asks *which* module before writing anything. The picker is
  fed from hints, so it offers what the project already has plus the whole plugin catalogue, and picking
  **pre-writes** a command rather than running one. The command only *creates* the module; `runCommand`
  splices it into the chain once the create comes back clean (`moduleInsert.ts`), so a rejected name never
  splices a module that was never written. Any other command in between means the user moved on, and the
  pending insert is dropped. With no hints there is nothing to show, so it falls back to pre-writing the
  command with the two ends and letting the bar's completion take over.

  Either way the chain names the *node* by its FILENAME alone, so that name is the one thing only the user
  can supply — which is why both paths stop at the bar. From a namespace it takes one command: a chain
  cannot name `@engine/…` (`use` takes a local module), so it creates the local module that inherits it
  *and* the node, with the module name pre-filled and the node name left to you.

- **`rec <track>`** (the `r` key). The lane's A button, verbatim: `onTrackToggle(id, "arm")`, which arms an
  instrument track for MIDI capture in the browser and sends `transport arm` for any other. One key, one
  button, one function, so the two cannot route a lane differently.

## Mobile

A phone shows one page, as the desktop does, and the menu drives `full` where the desktop's pane row
does. The home page is the same one, the timeline being the one pane that says what the song *is*: the
editor needs a keyboard and the graph needs room.

Every row of chrome is a row the panes do not get. On a phone in landscape the whole window is about 400px
tall, so the floating bar and the footer are trimmed to what a touch target needs and no more. The one control that
could not simply move is the command bar's opener: a phone has neither Ctrl+Shift+P nor double-Shift, so the menu carries it.

**The shell's root clips its overflow** (`overflow-hidden` on `App.tsx`'s outer box). A pane
wider than the screen — the DJ desk in portrait wants about 810px of a 420px phone — would
otherwise widen the DOCUMENT, and a phone's browser answers a wider document by growing its layout
viewport to match: every `fixed` box, every dialog and sheet, is then laid out in an 810×1650 page
the screen shows the top-left corner of. The song browser opens half off the screen, and a tap on
its row lands on whatever the screen has at that point instead. A pane that overflows is cut at the
edge; it never moves the dialogs.

Being a phone is a question of SIZE, not of pointer (`useIsMobile.ts`): an iPad gets the desktop
layout. What a touch screen of any size does need is the right-click, and a finger held still is one
(`longPress.ts`, [contextMenu.md](contextMenu.md)).

**A field is 16px on a coarse pointer** (`index.css`, unlayered so it beats every size utility): iOS
WebKit zooms the page into any focused field under 16px and leaves it zoomed. The phone's overflow
menu is a plain dialog of buttons, not an ARIA menu: it takes focus on opening, on the pane in use, and
every segment of it is 44px.

## Keyboard focus is always drawn

**Tab and Shift+Tab are the browser's**, everywhere in the shell: `keyToken` makes no token of Tab, so
no binding can take it and a stored `key: tab` is read as no assignment. A keyboard-only or
screen-reader user walks the floating bar, the transport, the page's controls (a lane's M/S/A among
them), the footer's panel tabs and the settings in document order, which is what a key bound to Tab
would forbid. The one component that keeps Tab is a field of the command bar, a form whose fields it
steps through ([commandBar.md](commandBar.md)).

The timeline is no tab stop: its caret is the shell's own keys, which answer wherever focus is, as the
rule for drag surfaces below says.

`index.css` gives every button, link, `role="button"`/`option` and `tabindex` element one focus ring
under `:focus-visible`, at zero specificity (`:where`) so a component's own ring still wins. A field
that removes the outline (`outline-none`) puts a cue of its own in its place — a border that turns the
primary colour, or a ring under `focus-visible:` — never nothing. The chains graph injects its own
sheet, so it carries the same rule (`apps/k7-daw-frontend/packages/ui-chains-react/src/chainsCss.ts`).

A selection ROW — a node card, a corner button drawn as a `span` — is focusable and answers Enter as
it answers a click. Not Space: Space is the transport wherever focus is ("Space is the transport, wherever the last click
landed", above), and a row that took
it would stop the music for whoever had just clicked a card. The command bar's list is a listbox the
field drives (`aria-activedescendant`), so its rows are never in the tab order. A drag or scrub
SURFACE (the ruler, a waveform, the roll's grid, a lane's body, a clip) is not focusable either: its
keyboard is the shell's own keys, the timeline's caret among them, which reach the same commands.

## The service worker, and the scope it cannot avoid

**The worker is the SHARED preset's** — every frediverse app gets one, from `@studio/vite-config`, whose
`docs/pwa.md` holds the rationale for `registerType`, `clientsClaim`, `devOptions` and why no manifest is
generated. `appMain.tsx` registers it.

**What it buys is a second opening, not offline.** The shell and its assets are precached, so k7 starts
from disk instead of re-fetching a large bundle over the LAN. It buys nothing offline, and that is not an
omission: the editor reads its project from the file service and its sound from the engine, both on the
musician's machine, so a cached shell with no machine behind it can only draw the "aucune machine" it
already draws.

**The scope is `/`, the whole origin, and it cannot be narrower — this is the part k7 owns alone.** A
worker's scope is a prefix and k7's own routes ARE the root, `/@local/…`. So this worker also
controls `/api` and `/auth`, which are k7's own routes,
so the worker serving them is right. The only thing keeping k7's honest is the `pwaDenylist` it
passes to `defineAppConfig` (`OTHER_APPS` in `vite.config.ts`): without it, a navigation to one of
those paths is answered with k7's `index.html`, and the editor's router refuses an address it
cannot parse. **A path served on this origin by anything but k7's shell belongs in that list.**

**Mixed content is what makes an install fail.** A page over HTTPS that loads one resource in the clear is
flagged "includes resources that are not secure", and that alone makes it uninstallable — the certificate
can be perfect and the manifest complete. The engine probe is where it comes from here, when it lists LAN
addresses as `ws://` candidates and lets the browser refuse them. See `@studio/engine-browser`'s
`docs/access.md`.

## A new build replaces the old one

Every deploy reaches every open tab, by the shared preset's `register` entry (`docs/pwa.md` in
`@studio/vite-config`): an editor tab stays open for days, and one running a bundle from before
a protocol change talks to a server that no longer answers it — `http://127.0.0.1:undefined` was
one. What makes the hidden reload affordable here is that the editor autosaves.

## The PWA manifest uses RELATIVE urls, deliberately

`public/manifest.webmanifest` has `"start_url": "."` and unprefixed icon `src`s, and neither is an
oversight. A manifest's URLs resolve against **the manifest's own address**, and Vite does not rewrite the
contents of a public file the way it rebases `index.html` — so anything written `/…` there is
origin-absolute and escapes the app.

An origin-absolute `"start_url": "/"` therefore installs whichever app owns the root, which is k7 today
and need not stay so. `.` is right under any mount; `/` is right under exactly one. A manifest that has to
be revisited when the edge's path table moves is a manifest written wrong.

## Where the rest of it went

| piece | file |
|---|---|
| the pane/view/panel tables, transport clock | `src/shellMeta.tsx` |
| the palette catalogue and argument completion | `src/palette.ts` |
| the shell's own commands — row and handler, one entry each | `src/ui/commands/` |
| every popin, dialog and menu drawn over the panes | `src/shell/ShellOverlays.tsx` |
| the header, its git capsule, the floating bar, the phone menu | `src/shell/ProjectHeader.tsx`, `HeaderGit.tsx`, `FloatingBar.tsx`, `OverflowMenu.tsx` |
| the panel registry and its frame | `src/shell/panels.tsx` |
| the git screens' one entry | `src/git/gitScreens.tsx` |
| what the graph's gestures run — the module picker, the asset browser, a tap's edit | `src/shell/useGraphActions.ts` |
| self-contained slices of state | `src/hooks/` — engine link, project doc, track controls, polls, logs, MIDI recording, dialogs |
