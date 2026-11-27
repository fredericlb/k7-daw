# Background processes — the running spinner

At the app's top right, a spinner shows while anything runs: a transcription, a chain bounce or a
separation, a picture render or a video's copies, an export (`shell/RunningBubble.tsx`). It is a button
named by the running count; a press opens a bubble listing each process with its command line, how far
it is when it says so and how long it has run. With nothing running there is no spinner. What runs is
asked about where the eye already is, at a glance, so it is a bubble and not a panel to open and close.

**An ending is a toast**, once, for a process this page saw running (`watchProcessEnds`, wired in
`App`): "done" for a success, and for a failure an error toast that names why and waits to be
dismissed, so the reason is still readable after the spinner has gone. One first seen already ended —
a page opened after it — says nothing: it is not news. No list keeps ended work.

## Two sources, one list

`src/processes.ts` merges them by start time:

- **The file service's**, mirrored from the `processes` field of the `/workspace` answer
  (`apps/k7-daw-backend/packages/k7-file-service/src/runtime/list.rs`, `apps/k7-daw-backend/docs/file-runtime.md`). That poll
  already runs every two seconds for the take and the engine's load, so the list costs no request of
  its own, and every tab of the account sees the same processes.
- **This page's own**, `trackProcess`: what runs in the browser or behind a route rather than a
  command, which no poll reports. Today that is the video export (`exportScene`, frame by frame) and
  the mixdown export. These rows say "in this tab", since another tab does not see them.

## What counts as a process

A **command still running after a second**, by its line. The threshold keeps an ordinary edit from
flashing the button; anything long enough to wait on is a command already (`track transcribe`,
`track process`, `clip process`, `playlist stems`), so listing commands covers them without each verb
opting in. A refused command ends as failed, with its last line as the reason.

**The work a verb starts and does not wait for** (`h.background`: `picture prepare`, `picture render`),
by its tag, the absolute path in it shortened to the file's name.

Progress is said where it is known: a render's frames, a separation's percent and phase, an export's
frames. Without it the row is a spinner and a clock.

## Not listed

- **The engine's load, copy and plugin scan.** They hold Play and have their own bar at the bottom
  (`StartupBar`); listing them twice would say the same wait in two places.
