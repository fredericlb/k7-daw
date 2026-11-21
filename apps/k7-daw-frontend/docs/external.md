# One surface, on another device

A surface is a screen the project declares. Some are things you use **while walking away from the
computer** — a desk you want on a tablet propped beside the decks, not on the laptop behind. Every
device on the network that has a project open lists its surfaces in the **Surfaces** tab of the home
page, and opening one gives **that surface and nothing else**, driving the same engine as the device
that has the project open (the *main* device).

```
/@lan/<engineId>/<workspace>/<project>/@external/<surface>
<project>/@external/<surface>                      the same surface, on the machine that holds the project
```

How the devices find each other and what the main device opens for it is
`apps/k7-daw-backend/docs/lan.md`; this page is what the page draws.

## Choices

| choice | why |
|---|---|
| the page talks to its own front only; the front relays to the other device | Android allows cleartext to loopback alone, and the other device's token never reaches the page's script |
| the address names the device by its engine id, under a reserved repository `@lan` | a device's name changes and two can share one; the id is what its engine proves |
| the surface sits BELOW the project, behind an `@` sentinel (`@external`) | `parseLaunch` ends a project at the last segment carrying the app's suffix, so what follows is already the app's route and nothing else reads it; `@` cannot start a workspace name nor be a surface id (`createSurfaceFile` takes letters, digits, `_` and `-`) |
| the remote shows the surface and no way to change it, but keeps the bottom transport | a tablet by the decks is where the hand is; walking back to the laptop to press play is what the screen exists to avoid |
| one round Home button stays, a hard navigation to the Surfaces tab | an installed app has no address bar and no back; without it the only exit is killing the app. Going home is not changing surface, and picking another surface is a separate act, from the tab |
| a remote claims nothing, and its gate opens no project | a claim from a second device would buy nothing, and the open sequence would run on the main device and could interrupt what plays |
| the main device cannot open the pane a remote shows, but a remote may take the pane the main is on | the remote is the screen the person just walked to; the main moves to the next pane that is free and says so |
| two remotes may show one surface | the user restricted only the main device; the row reads "Also on <device>" |
| `/external` does not exist | the front answers its own machine only, so a browser on a second device has no address to reach, and k7 runs on devices |

## The Surfaces tab

`/surfaces` is a route of the home, after Projects. A group per device — its name over its project's name —
with a row per surface: the type's glyph and word, the surface's id, and on the right nothing, "Also on <device>"
or the one reason the row does not open. Devices sort by name, surfaces by their order in `surfaces/`. A device appears
once its hello answered, so the list does not grow line by line; a device that never answers reads "Not answering".

The filters run in a fixed order and a failed one disables the row instead of hiding it
(`src/surfaces/surfaceRows.ts`): reached, trusted (the engine gave this device its token), compatible (the door's `api` and
the engine's protocol agree, else "Update k7 on <the older device>"), drawable (`deviceSurfaceDrawer` has the type, else
"Drawn in k7's window only"). This device's own engine is never listed, and neither is the project this device has open: the
editor is where its own screens open.

Three empties, three different facts: *No device found* (nobody is advertising: turn on **Available on the local network**
on the device with the project), *<device> has no project open*, *<device>'s project declares no surface*.

A row is a real navigation (`<a href>`), since the surface mounts under a project basename the home's router cannot reach.

## The remote view

`LanSurfaceGate` reads the other device's hello, points the file transport at its door through this front
(`/api/lan/<id>/files`) and mounts `ExternalSurface` with the engine socket relayed the same way. When the project on that device
is closed or another one is open, it draws a card with the way home.

`ExternalSurface` has two shapes. On its own machine the transport is `MiniTransport` at the head and the foot carries a link
back to the tab. As a remote the transport is the bar's own capsule (`FloatingBar` with `surfaceOnly`: play, stop and the clock,
no tool, no views, no command button) at the foot, and the round Home button sits top-left. The command line and full screen stay
in both. A remote beats a presence line to the main device every five seconds and says goodbye as the page goes
(`surfaces/presence.ts`), which is what blocks the pane there.

## The exclusion, on the main device

`surfaces/paneInUse.ts` maps a remote's surface to the control it is on the main device: the timeline pane, the desk pane or the
picture window; a mastering surface has no pane there, so nothing of it is blocked. A blocked pane in the header's switcher is
dimmed and says "In use on <device>" on a press (the kit's `disabledReason`), the `ui` pane verbs and the picture window refuse
with the same sentence in the status line, and the polling that feeds it is every two seconds while a project is open. A pane
comes back within two seconds of the remote's goodbye, or 15 seconds of a crash.

## What a surface is made of

None of it is the shell with things hidden. `ExternalSurface` mounts what a surface needs — the engine link, the desk, the video
document, the command runner — and hands over to the component the type names; the shell's panels and its bar all belong to a
screen this device does not show. Those hooks exist precisely so a host can wire a surface (`../packages/k7-plugins/docs/turntables/hosting.md`).

## The transport

On its own machine, `MiniTransport` (`src/shellMeta.tsx`) at the head: play/pause and the clock, nothing else — stop, rewind, record and the metronome are
decisions about a session you are looking at, and the keys do all of them anyway.

As a remote the transport is the bar's own capsule at the foot (`FloatingBar` with `surfaceOnly`): play, stop and the clock.
A tablet by the decks is where the hand is; walking back to the laptop to press play is what this
screen exists to avoid.

The clock subscribes to `playhead` by itself (`shellMeta.tsx`): at ~20 Hz, a subscription taken at
this level would redraw the whole surface every frame.

## The keyboard, whole

`useCommandKeys` is the shell's listener, shared with it — same withdrawal rules (a field being typed
in owns its keys, a focused button gives back Space and Escape), same ⌃C exception on a text
selection, same **double-Shift**. One keyboard cannot become two.

The keymap is the person's: `effectiveKeys(commandPrefs)`, with the preferences read from
`engines.prefs()` as in the shell. A remapped play key has to work on the tablet too, or the two
screens are two instruments.

Double-Shift opens the **command line** at the foot — what the gesture means here, as it focuses the
anchored bar or opens the palette in the shell. The host supplies the gesture, the listener only
recognises it.

What the surface answers **itself**, before `/command`:

| lines | why |
|---|---|
| `transport play` / `stop` / `toggle` / `rewind` | over the socket, as in the shell: the file service would send that message after an HTTP round trip and a walk of the undo journal |
| `transport replay` | a seek to the start of the current run, then the bare verb |
| `transport locate`, `transport loop`, `ui select`, `ui zoom …`, `ui ruler`, `ui lanes`, `ui automation` | the cursor and the zoom belong to the mounted pane (`TimelineHandle`); no service can move them. On a surface with no arrangement, the line **says so** — the key is bound, the command exists, it is the device that is the wrong place |
| any other `ui …` | names a pane, a panel or a screen this device does not have — refused **in words**, rather than posted to a service that would answer "unknown command" |

The **layout** verbs (`ui close`, `ui global`, `ui focus next` and `prev`, the two closes) are the one exception
to that last line: they stay silent. That is what Escape and F6 come to when there is nothing to
close, and a screen answering Escape with a sentence about the editor is a screen that grumbles
every time you dismiss something.

The conditions (`caret_track`, `timeline_selection`, `timeline_zoomed`, `timeline_loop`) come up from
the mounted surface. The shell draws them from its own state; here there is one surface and it holds
them all. Without them, `m` and `s` would send an empty-argument command instead of staying quiet.

## All four types drive

| type | drawn by | what the host gives it |
|---|---|---|
| `turntables` | `SurfaceTab` | the desk (`useTurntableDeck`) |
| `mastering` | `MasteringTab` | the engine link: `setPluginParam`, `watchPluginParams`, the param store |
| `viewport` | `ViewportTab` | the compiled picture (`useVideoDoc`) |
| `timeline` | `ExternalTimeline` | everything else — see below |

None of the four asks for the shell, which is what makes them all drivable here:

- `mastering` asks only for **an engine link**, which this screen already has.
- `viewport` asks only for a **video document**, which the browser Worker answers.
- `timeline` asks for more, but what it draws on — the cursor, the zoom, the selection — does not
  belong to the shell: it belongs to the **pane**, and `TimelineTab` holds them itself. What the
  shell really supplies is writing: the commands, the menus, the loop. `ExternalTimeline` rewires
  those without it.

So no surface has only an address, and there is one dead-end screen: the address names no file in
`surfaces/`.

### The timeline, on a tablet

The same `TimelineTab` as the shell's pane, with the same write gestures, each to the same command
line:

| gesture | line |
|---|---|
| move a clip | `event move` |
| drag a handle | `event resize` |
| drag a fade corner | `event fadein` / `event fadeout` |
| the corner button | `track new` |
| mute / solo / arm | `track mute`, `track solo`, `transport arm` (`useTrackControls`) |
| move a picture | a **file write**, not a command — the CLI has no verb for placing a picture (`useVideoEdits`, `picture.md`) |

Three things come from somewhere other than the shell, because there is none:

`useSurfaceCommands` is the runner — `/command`, a reload key, a status line, and a hook for what the
screen answers itself (the table above). It asks `want: []` every time: the graph the service can
build on the way is for the chains canvas, and there is none here. Its reload key follows the
**`/revision` token** and not only our own writes — a tablet is a second screen on a project also
being edited on the laptop, so most of what it must follow is not its own.

The **graph** is refetched here, on that same token, where the shell takes it from the chains pane:
without it the gutter would draw neither the alternatives badge nor the automation curves.

The **context menu** is the shell's, `contextMenu.ts`, and the gutter badge's menu is `nodeMenu`,
shared with it: two derivations of one menu are two menus that end up offering different
alternatives on the same node.

### A line, and not a command bar

A menu entry is either **complete** — the click filled all its arguments, it runs — or it still owes
one, and in the shell those go into the bar half-written. There is no bar here. Dropping them would
have removed "New alternative…" from a menu that is entirely about that; building one would have
rebuilt the shell.

So the foot carries **one line**, filled by those entries, opened by double-Shift, and typable in its
own right. No completion, no palette, no catalogue behind it: what makes those three useful is a
keyboard — but the line itself is useful exactly when there is one, or when a menu has just dropped a
command to finish into it.

It is at the foot of the HOST and not of the timeline: double-Shift has to answer on a desk as on an
arrangement, and a menu opened on any surface needs somewhere to drop its line.

### What is not offered

**MIDI** capture. It happens in the browser holding the keyboard, and that one is across the room;
`transport arm` does work, because that is arming the **engine**. And `trackIds` is not passed to the
`viewport`: that list indexes the engine's `levels` array for a shader with an audio input, which
almost none have — passing it would mean compiling the song on a device that only shows its picture.

## `ui external`

`ui external <surface>` copies the address of `<project>/@external/<surface>` to the clipboard and writes it into the status line, so
it is readable even if the browser refuses the write. It opens the surface alone in a browser on THIS machine, and it does not
navigate: following it would replace the editor you are standing in front of. It is built from `window.location.href`, like every
project address: a local copy and the public host are different answers, and only the tab knows which one it is reading. A surface
on ANOTHER device is opened from the Surfaces tab, never by an address carried across.

## Wake lock and the camera

The page on a device is `http://127.0.0.1:…`, a secure context, so the wake lock and the camera work on the remote as on the main
device. A lane that names the main machine's lens draws nothing on a remote, since a camera is matched against the browser it
opens in; a `remote:` camera, a device of the account in camera mode, arrives over a peer connection and draws
(`apps/k7-daw-frontend/packages/song/docs/camera.md`).

## What this is not

It is not a read-only copy. A remote surface **writes** — a deck loaded from the tablet is loaded for everyone, because it is
the same engine and the same session.
