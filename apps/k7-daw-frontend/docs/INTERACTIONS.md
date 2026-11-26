<!-- Generated from the Sheet https://claude.ai/artifact/GWymnBJpibPCE4b32ZR1gz by scripts/interactions/build.mjs: do not edit by hand. -->
# K7 DAW interactions

This inventory was read from main's source code on 2026-11-20. Nothing in it was tested on a device.

Each interaction is listed once, under the first screen below that has it. "—" means that input has no way to do it.
On touch, a long-press (500 ms, held still) acts as a right-click wherever a right-click menu exists.
⌘/Ctrl means Cmd on macOS and Ctrl elsewhere.

## Identifiers

A screen is a row of the design canvas (an iPad, an iPhone and a Desktop board), numbered top to bottom; each device variant of a screen has its own identifier: `T` iPad (tablet), `M` iPhone (mobile), `D` Desktop. A screen with no board is listed here as not drawn.
An interaction is `<screen number>.<nn>` (`03.05`), one id shared by every device so the three variants converge on the same behaviour. The Devices column says where it applies: `D` when a mouse, trackpad or keyboard can do it, `T` when touch or an iPad keyboard can, `M` when touch can. A row lives on the first screen that shows its element; global, header, transport, command bar and context-menu rows are not repeated on later screens.
† marks a row whose element is not drawn on that screen's boards (it is kept on the screen of its area). The counts in the table are the rows each device can do.

| Screen | Tablet | Mobile | Desktop | Canvas board(s) |
|---|---|---|---|---|
| 01 Home | 01T (41) | 01M (37) | 01D (43) | Artboard-hb0q, HomePhone, HomeDesk, HomeEmptyRemotePad, HomeEmptyRemotePhone |
| 02 Tasks | 02T (4) | 02M (4) | 02D (4) | TasksPad, TasksPhone, TasksDesk |
| 03 Arrangement · Audio | 03T (81) | 03M (70) | 03D (90) | ArrangePad, ArrangePhone, ArrangeDesk |
| 04 Arrangement · Video | 04T (8) | 04M (8) | 04D (9) | ArrangeVideoPad, ArrangeVideoPhone, ArrangeVideoDesk |
| 05 Graph | 05T (50) | 05M (48) | 05D (53) | GraphPad, GraphPhone, GraphDesk |
| 06 Source | 06T (23) | 06M (20) | 06D (25) | SourcePad, SourcePhone, SourceDesk |
| 07 Resources panel | 07T (7) | 07M (7) | 07D (7) | ResourcesPad, ResourcesPhone, ResourcesDesk |
| 08 Tasks & history panel | 08T (9) | 08M (9) | 08D (10) | LogPad, LogPhone, LogDesk |
| 09 Export panel | 09T (6) | 09M (6) | 09D (6) | SharePad, SharePhone, ShareDesk |
| 10 Settings panel | 10T (5) | 10M (5) | 10D (5) | SettingsPad, SettingsPhone, SettingsDesk |
| 11 Clip editor | 11T (27) | 11M (27) | 11D (27) | ClipZoomPad, ClipPhone, ClipZoomDesk |
| 12 Piano roll | 12T (22) | 12M (21) | 12D (26) | PianoSidePad, PianoPhone, PianoSideDesk |
| 13 Node chain editor | 13T (10) | 13M (10) | 13D (10) | ChainSidePad, ChainPhone, ChainSideDesk |
| 14 Export · folder picker | 14T (0) | 14M (0) | 14D (0) | ExportPickPad, ExportPickPhone, ExportPickDesk |
| 15 Export · saved | 15T (0) | 15M (0) | 15D (0) | ExportDonePad, ExportDonePhone, ExportDoneDesk |
| 16 Pull in place | 16T (1) | 16M (1) | 16D (1) | PullPad, PullPhone, PullDesk |
| 17 Takes mode | 17T (11) | 17M (11) | 17D (12) | TakesPad, TakesPhone, TakesDesk |
| 18 Command palette | 18T (3) | 18M (3) | 18D (4) | CommandPad, CommandPhone, CommandDesk |
| 19 Command palette · argument | 19T (4) | 19M (4) | 19D (4) | CommandArgPad, CommandArgPhone, CommandArgDesk |
| 20 Turntables | 20T (27) | 20M (27) | 20D (27) | TurntablesPad, TurntablesPhone, TurntablesDesk |
| 21 Library | 21T (6) | 21M (6) | 21D (6) | LibrariesPad, LibrariesPhone, LibrariesDesk |
| 22 Frozen chains | 22T (0) | 22M (0) | 22D (0) | not drawn |
| 23 App settings | 23T (25) | 23M (25) | 23D (25) | AppSettingsPad, AppSettingsPhone, AppSettingsDesk, AppSettingsDownloadsPad, AppSettingsDownloadsPhone, AppSettingsDownloadsDesk, AppSettingsAboutPad, AppSettingsAboutPhone, AppSettingsAboutDesk, AppSettingsGitHubPad, AppSettingsGitHubPhone, ServicesForgejoPad, ServicesForgejoPhone, ForgejoAddPad, ForgejoAddPhone |
| 24 Logs panel | 24T (2) | 24M (2) | 24D (2) | LogsPad, LogsPhone, LogsDesk |
| 25 Playlists panel | 25T (12) | 25M (12) | 25D (13) | PlaylistsPad, PlaylistsPhone, PlaylistsDesk |
| 26 Command editor screen | 26T (10) | 26M (9) | 26D (10) | CommandEditorPad, CommandEditorPhone, CommandEditorDesk |
| 27 Mixer panel | 27T (4) | 27M (4) | 27D (4) | MixerPad, MixerPhone, MixerDesk |
| 28 Events panel | 28T (1) | 28M (1) | 28D (1) | EventsPad, EventsPhone, EventsDesk |
| 29 Sync settings | 29T (6) | 29M (6) | 29D (6) | ConnectPad, ConnectPhone, ConnectDesk |
| 30 Workspace history | 30T (3) | 30M (3) | 30D (3) | SyncHistoryPad, SyncHistoryPhone, SyncHistoryDesk |
| 31 Conflict | 31T (5) | 31M (5) | 31D (5) | ConflictPad, ConflictPhone, ConflictDesk |
| 32 Host key prompt | 32T (2) | 32M (2) | 32D (2) | HostKeyPad, HostKeyPhone, HostKeyDesk |
| 33 Saves list | 33T (2) | 33M (2) | 33D (2) | CommitsPad, CommitsPhone, CommitsDesk |
| 34 Save | 34T (4) | 34M (4) | 34D (4) | CommitPad, CommitPhone, CommitDesk |
| 35 Workspace tree | 35T (4) | 35M (4) | 35D (4) | not drawn |
| 36 Engine page | 36T (11) | 36M (11) | 36D (11) | not drawn |
| 37 Help | 37T (3) | 37M (3) | 37D (3) | not drawn |
| 38 Curves | 38T (15) | 38M (15) | 38D (15) | not drawn |
| 39 Chain and module builders | 39T (26) | 39M (24) | 39D (27) | not drawn |
| 40 Surfaces | 40T (8) | 40M (8) | 40D (8) | SurfacesPad, SurfacesPhone, SurfacesDesk, RemoteSurfacePad, RemoteSurfacePhone, RemoteInUseDesk |
| 41 Forge sign-in | 41T (10) | 41M (10) | 41D (10) | GitHubWaysPad, GitHubWaysPhone, DeviceFlowPad, DeviceFlowPhone, DeviceFlowExpiredPad, DeviceFlowExpiredPhone, GitHubPermissionPad, GitHubPermissionPhone, ForgejoSignInPad, ForgejoSignInPhone |
| 42 Forge repositories | 42T (7) | 42M (7) | 42D (7) | GitHubPickerPad, GitHubPickerPhone, ConnectAccountsPad, ConnectAccountsPhone, NewRepoPad, NewRepoPhone |
| 43 Clone a repository | 43T (2) | 43M (2) | 43D (2) | CloneWaysPad, CloneWaysPhone, CloneForgejoPad, CloneForgejoPhone, CloneHostPad, CloneHostPhone |

## 01 Home

### Global (every screen)

Applies on every screen: the keymap, the Esc ladder, the undo journal, app questions, toasts, engine strips and dialogs, floating windows.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 01.01 | Open the command bar | click the command-bar button (square terminal): the floating bar's left in a project, the bottom right of the sidebar on Home | ⌘/Ctrl+⇧P (works inside the text editor); double-tap Shift | double-tap the screen with two fingers, anywhere; a tablet also has the command-bar button (the floating bar's left in a project, the bottom right of the sidebar on Home); a phone has no button | T M D |
| 01.02 | Run a bound command by key | — | single keys from the keymap, each one runs a command: `1`–`4` pages, `f` fullscreen the focused pane, `F6`/`⇧F6` next/previous page, `?` help, `Space` play/stop, `⇧Space` replay, `Enter` rewind, `t` locate, `,` `.` step the playhead, `⇧R` record enable; every key can be rebound | — | T D |
| 01.03 | Step aside for typing or a focused control | — | a field owns every key; a focused button/link/select keeps all keys but Esc and the transport keys; `F6` walks out of both | — | T D |
| 01.04 | Copy selected page text | — | ⌘/Ctrl+C with a text selection stays the browser's copy | — | T D |
| 01.05 | Block the trackpad's back swipe | a two-finger sideways swipe that nothing scrolls is cancelled | — | — | D |
| 01.06 | Drop files from the OS | drag files over a drop target (timeline lane, workspace row); internal drags are refused | — | — | D |
| 01.07 | Answer an app question (text or confirm) | click an answer; click the backdrop or Cancel to cancel | Enter answers (last button is the default), Esc cancels; a tool key (Space in the tempo question) presses the tool button | tap the buttons | T M D |
| 01.08 | Dismiss a toast | click the ×; hovering or focus holds it; the engine-offline toast has no × while the link is down | — | tap the × | T M D |
| 01.09† | Approve or deny a device asking to pair | click Approve or Deny on the prompt or card | — | tap | T M D |
| 01.10† | Restart the engine when it is lost | click Restart on the engine-offline toast (it cannot be dismissed while the link is down) | — | tap | T M D |
| 01.11† | Take the engine back from another device | click the banner's button | — | tap | T M D |
| 01.12† | Import a .dawproject | click Import / Open imported / Cancel (locked mid-import) | Esc cancels when idle | tap | T M D |
| 01.45 | Read why a control is disabled | click it: it stays pressable, drawn dimmed, and a toast gives the reason | Enter / Space on it | tap it | T M D |

### Home and its pages · Projects

The home pages behind a floating tab bar (Projects, Surfaces, Tasks, Library, Settings) with the round command-bar button: workspace cards with their project rows, Pull and Save (Setup sync on a workspace with no remote), a filter field once the list is long, the projects ⋮ menu (Show examples, Help). The Library page is read-only and holds the frozen chains.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 01.14 | Open a project | click the row (not its ⋮) or its name link; middle-click opens a new tab | Tab to the name link, Enter | tap the row | T M D |
| 01.15 | Create a project | click "New project" (page header or a band's button) → name and archetype dialog | Enter creates; Esc cancels; arrows pick the archetype | tap | T M D |
| 01.16 | Pick the target workspace for a new project | click a workspace button in the dialog | — | tap | T M D |
| 01.17 | Create a workspace | click "New workspace" (also the empty page's main action) → name dialog | Enter creates; Esc cancels | tap | T M D |
| 01.18 | Clone a workspace from git | click "Clone from git" (the New menu beside Projects; the empty page's second button) → the Clone a repository page (`?clone`), tabs GitHub / Forgejo / Other host, opening on the first that can list repositories, else on Other host (see Git screens) | — | tap | T M D |
| 01.19 | Clone a workspace known from another device | click Clone on a row of "On your other devices" → the Clone a repository page on Other host, the address and the name filled in | — | tap | T M D |
| 01.20 | Open a workspace's file tree | click the workspace name on its band | Tab, Enter | tap | T M D |
| 01.21 | Fold / unfold a workspace band | click the chevron | Enter/Space | tap | T M D |
| 01.22 | Create the first project of an empty workspace | click "Create the first project" | Enter | tap | T M D |
| 01.23 | Filter projects | type in the Filter projects field (shown once there are 6 projects or more) | Tab to the field, type | tap, type | T M D |
| 01.24 | Open a project's menu (copy, rename, delete) | click ⋮ on the row | Enter on ⋮, arrows, Enter | tap ⋮ | T M D |
| 01.25 | Rename a project | ⋮ → Rename → name dialog | Enter confirms; Esc cancels | tap | T M D |
| 01.26 | Copy a project to another workspace | ⋮ → Copy → pick the workspace, set the name | Enter confirms; Esc cancels | tap | T M D |
| 01.27 | Delete a project | ⋮ → Delete → click the red Delete | Esc cancels | tap | T M D |
| 01.28 | Open a workspace's menu (Sync settings, delete) | click ⋮ on its band (connected, the menu names the remote: "Connected to host/repo") | Enter, arrows | tap ⋮ | T M D |
| 01.29 | Delete a workspace | ⋮ → Delete… → type the workspace's name, confirm | type, Enter; Esc cancels | tap | T M D |
| 01.30 | Open a workspace's Sync settings | ⋮ → "Sync settings…", or "Setup sync" on the card of a workspace with no remote → the Sync settings page (`?sync=<workspace>`), tabs GitHub / Forgejo / Other host, opening on the first that can list repositories; on Other host when the workspace has a remote (shown there) or no account can list | — | tap | T M D |
| 01.31† | Import a project file by dropping | drag a `.dawproject` file onto a workspace's row (it lights up) → name dialog | Enter confirms | — | T D |
| 01.32 | Show or hide the Examples workspace | page ⋮ → "Show examples" (remembered per device; it cannot be hidden while it is the only workspace) | Enter | tap | T M D |
| 01.33 | Go to Libraries from the missing-library notice | click the notice's link | Enter | tap | T M D |
| 01.34 | Switch Home page | click a tab of the floating tab bar: Projects, Surfaces, Tasks, Library, Settings (each a link) | Tab, Enter | tap a tab | T M D |
| 01.35 | Open Settings, the Engine page or About | click the Settings tab; Devices → Engine opens the Engine page; About is its own tab | Tab, Enter | tap the Settings tab | T M D |
| 01.37† | Open the engine menu, choose where audio plays | click the engine light; "Engine settings", "This machine" or "The engine" | ArrowDown/Enter in the menu | tap | T M D |
| 01.38 | Run a Home command (new workspace, delete, camera mode…) | type in the command bar | Enter | same | T M D |
| 01.39† | Stop camera mode | click Stop / ✕ on the camera overlay | — | tap | T M D |
| 01.40† | Switch the camera lens | click the switch-lens button | — | tap | T M D |
| 01.44 | Open Help from Home | page ⋮ → Help, or the Help command in the command bar | Enter in the menu | tap | T M D |

### Git screens

The git bar (workspace tree, Home row, project header), Save in one press with its toasts, the Conflict screen, the Sync settings and Clone a repository pages (Back to Projects; tabs GitHub, Forgejo, Other host; one repository picker per signed-in forge account), the Saves list, the identity form; GitHub's sign-in is in Settings → Services, and a Forgejo server is added and signed in there or in the Forgejo tab.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 01.41 | Save | click Save on the workspace card ("Save, N changes") or the tree's bar → one press, no dialog: everything is committed as "Saved on dd/mm/yyyy hh:mm", then sent; a toast says Saving…, then Saved | — | tap | T M D |
| 01.42 | Pull | click Pull (dimmed when nothing to pull; over unsaved changes it reads "Save first", dimmed) | — | tap | T M D |
| 01.43 | Send the saves waiting | click Save with no change and saves waiting: it only sends them ("Sent to <remote> · <branch>"); with no remote, Save only records | — | tap | T M D |

## 02 Tasks

### Home and its pages · Tasks

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 02.01 | Show or hide done tasks | tick the checkbox | Space | tap | T M D |
| 02.02 | Refresh the list | click refresh | Enter | tap | T M D |
| 02.03 | Open a task's workspace | click "Open workspace" on its heading | Enter | tap | T M D |
| 02.04 | Open the project a task is anchored to | click the `project @ timecode` link | Enter | tap | T M D |

## 03 Arrangement · Audio

### Global (every screen)

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.01 | Back out one layer (Esc ladder) | — | Esc: pending MIDI learn, popin, graph slot, armed roll tool, waveform mode, held tool, fullscreen, then closes the editor dock | — | T D |
| 03.02 | Undo / redo the project journal | — | ⌘/Ctrl+Z undo; ⌘/Ctrl+Y or ⌘/Ctrl+⇧Z redo (the text editor spends its own history first) | — | T D |
| 03.03 | Save (shows the "saved automatically" notice) | — | ⌘/Ctrl+S outside the text editor | — | T D |
| 03.04 | Focus a pane (keys follow it) | click anywhere in a pane | `F6` / `⇧F6` | tap in a pane | T M D |
| 03.05 | Leave a project with unsaved work | click Back, then answer (the question names the act, and a recording still running) | Enter / Esc | tap | T M D |
| 03.06† | Reload after a failed engine sync | click Reload (Esc and outside clicks are ignored) | — | tap | T M D |
| 03.07† | Dismiss the missing samples/plugins dialog | click Dismiss or the backdrop | Esc | tap Dismiss | T M D |
| 03.08† | Confirm undoing a process step | click Confirm / Cancel | Esc cancels | tap | T M D |

### Project header and transport

The header (Back, the name over its status line, spinner, warning chip, git capsule, the pane switcher, the panel toolbar, Settings; on a phone the status line sits under it and the rest folds into the Views and panels dialog), the floating bar (command bar, tool, transport with Record's chevron, position, overview, tempo, metronome, master volume, the view's capsule) and the editor dock.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.10 | Go back to the projects page | click Back (middle-click: new tab); leaving stops and unloads the project in the engine | Enter on Back | tap Back | T M D |
| 03.11 | Play / pause | click Play/Pause | Space | tap | T M D |
| 03.12 | Stop and rewind | click Stop | Enter | tap | T M D |
| 03.13 | Replay from the last start | — | ⇧Space | — | T D |
| 03.14 | Toggle loop | click Repeat | ⇧L (selection → loop, again clears) | tap | T M D |
| 03.15 | Toggle record enable | click Record (arms a track first if none); where the project records its output, Record's chevron or a right-click opens Record the output… | ⇧R; ↓ on Record opens the output menu | tap; a long-press or the chevron opens Record the output… | T M D |
| 03.16 | Record the output (no arrangement) | click the output record button (a set); with an arrangement, Record's chevron → Record the output | — | tap | T M D |
| 03.17† | Move the playhead or scrub from the minimap | click the strip; press and drag to scrub | — | tap to seek; drag to scrub | T M D |
| 03.18† | Peek the full map | hover the minimap: it unfolds to every lane | — | — (stays folded) | D |
| 03.19 | Edit the tempo | click "N bpm" → question with a Tap button | Space taps, Enter confirms, Esc cancels | tap the bpm, tap Tap repeatedly | T M D |
| 03.20 | Edit the meter | click "N/M" → question | Enter / Esc | tap | T M D |
| 03.21 | Toggle the metronome | click the metronome button | Enter/Space on it (repeat ignored) | tap; on a phone, Metronome in Views and panels | T M D |
| 03.22 | Set the metronome volume | right-click the metronome button, drag the slider | ArrowDown opens it; arrows move it (written on key up) | long-press, drag the slider; on a phone, the slider under Metronome in Views and panels | T M D |
| 03.23 | Set the song's output level | drag the master volume in the floating bar (written on release) | arrows on the focused fader (0.5 dB) | drag the master volume; on a phone, the one in the info strip above the bar | T M D |
| 03.24 | Fix a clipped mix | click "fix" next to the clip light | — | — | D |
| 03.25† | Show / hide the graph notes | click the notes button | — | tap | T M D |
| 03.26 | Open the Mixer or the Events dock | click Mixer in the header's panel toolbar; opening a clip, a take or a roll opens Events | the key bound to `ui mixer` | tap Mixer in Views and panels | T M D |
| 03.27 | Close the editor dock | click Close editor in the dock header | Esc | tap Close editor | T M D |
| 03.28 | Resize the editor dock | drag the separator over its top edge (44 px tall, 120 px min); hold it 400 ms, then move up to expand the editor to full height or down to fold it back | ↑ / ↓ on the focused separator; Enter expands or folds | drag the separator; on a phone, hold the sheet's grabber, then move up to expand or down to fold | T M D |
| 03.29 | Open / close a panel (resources, playlists, logs, tasks and history, export, settings) | click its button in the header's panel toolbar (a pressed button shows its name); the same button or the panel's Close closes it | the key bound to `ui <panel>` | tap its tile in Views and panels (phone); tap Close | T M D |
| 03.30 | Switch pane (Arrangement / Graph / Source / Turntables) | click the pane switcher (its tooltip names the key); a pane another device shows is dimmed and says why | `1` `2` `3` `4`; `F6` / `⇧F6` | tap the pane in Views and panels (phone) | T M D |
| 03.31 | Show / hide the viewport window | click Picture: the viewport in Lane views (a project with a picture) | — | tap Picture in Views and panels (phone) | T M D |
| 03.32 | Open Views and panels (phone) | — | Esc closes; Tab stays inside it | tap the ⋯ button; tap outside or Close menu | T M D |
| 03.34 | Browse the file breadcrumb | click a segment → its menu; folder drills in, file opens, ▶ opens a project; click outside closes | — | tap | T M D |

### Context menus

The right-click menu on any entity: tabs per kind of thing, rows, a status line.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.35† | Open the menu on an entity | right-click | — | long-press (500 ms, hold still) | T M D |
| 03.36† | Pick a row | click (a complete row runs; a few ask first) | Enter on the focused row | tap | T M D |
| 03.37† | Walk the rows | hover (status line shows the description) | ↑ ↓ / Home / End | — | T D |
| 03.38† | Switch tab | click a tab icon | ← / → | tap | T M D |
| 03.39† | Close the menu | click outside, scroll, window blur, header ✕ (touch only) | Esc | tap outside or ✕ | T M D |

### Arrange · Ruler and loop

The timeline pane: tool strip, ruler with markers and task flags, timeline tabs, lane headers (M/S/A, alternative badge, meters), lanes with clips, picture lanes with shots, automation rows, reference bar and stems bar. Keys work while the pane is active. Clip, edge and shot drags snap to the grid: ⇧ frees, ⌘/Ctrl keeps whole grid steps from where the item was.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.40 | Seek from a ruler (also above the clip editors and takes) | click the ruler (snaps to the nearest drawn tick; clears the selection) | `t` locates to the caret | tap the ruler | T M D |
| 03.41 | Step the playhead one tick | — | `,` / `.` | — | T D |
| 03.42 | Select a time range across all lanes | drag sideways on the ruler | — | — | D |
| 03.43 | Zoom by dragging on the ruler | drag vertically on the ruler (up zooms in) | — | — | D |
| 03.44 | Add a marker | right-click the ruler → "Add a marker" | — | long-press the ruler | T M D |
| 03.45 | Loop the selection / clear the loop | right-click the ruler → "Loop selection" / "Clear loop" | ⇧L | long-press the ruler | T M D |
| 03.46 | Insert or remove time | right-click the ruler → "Insert time" / "Remove time" | — | long-press the ruler | T M D |
| 03.47 | Read a marker's name | hover its flag | — | — | D |
| 03.48 | Delete a marker | right-click the flag → "Delete a marker" | — | long-press the flag | T M D |
| 03.49 | Read a task's text | hover or focus the task flag (a click seeks) | Tab to the flag | tap (seeks) | T M D |
| 03.50 | Switch the ruler between bars and seconds | click the position in the floating bar; the rulers follow | — | tap | T M D |

### Arrange · Tracks

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.51 | Select a lane (audio or picture) | click its header | `h`/← from beat 1 onto the label; `j k`/↑↓ walk rows | tap the header | T M D |
| 03.52 | Pick several lanes | ⇧-click the headers | — | — | D |
| 03.53 | Group the picked lanes | right-click a picked header → "Group the N tracks" | — | long-press | T M D |
| 03.54 | Aim the other panes at a lane | double-click the header | — | double-tap | T M D |
| 03.55 | Reorder a lane | drag the header onto a row of the same kind | — | hold the header, then drag | T M D |
| 03.56 | Mute / solo / arm a lane | click M, S or A (A arms audio in the engine, MIDI in the browser) | `m` / `s` / `r` on the caret's lane | tap (32 × 16 inside a 44 px box on a touch screen); on a phone, second line when expanded | T M D |
| 03.57 | Mute / solo a group | right-click the group header → Mute / Solo | `m` / `s` on the group label | long-press | T M D |
| 03.58 | Track menu entries | right-click a lane → Move, Duplicate (with or without events), Rename, Add an input, Hide, Mute, Solo, Highlight 10 dB, Automate / Stop automating, Bounce through a chain, Copy to a timeline, Delete, Arm, Put a clip, Record notes into a sequence, Open the track's file | — | long-press | T M D |
| 03.59 | Add a lane | click "+ lane" (Audio / Video lane when the song has a frame) | `+` | tap | T M D |
| 03.60 | Switch a lane's attribute set | click the alternative badge → a set, New alternative…, Copy this one…, Freeze / Unfreeze | — | tap the badge | T M D |
| 03.61 | Expand / fold lanes | click Lanes in Lane views (disabled with a dock open) | — | tap | T M D |
| 03.62 | Show / hide automation lanes | click Automation in Lane views | — | tap | T M D |
| 03.63 | Add / move / remove an automation point | click an automation row to add a point; drag a point; right-click for the point menu | ↑ / ↓ change the focused point; Delete removes it | tap the row to add a point; hold a point, then drag | T M D |
| 03.64 | Put the caret on a lane cell | click the empty lane (clears the selection) | — | tap the empty lane | T M D |
| 03.65 | Walk the caret | — | ←/`h`, →/`l` a beat; ↑/`k`, ↓/`j` a row | — | T D |

### Arrange · Clips

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.66 | Select a clip | click it | — | tap | T M D |
| 03.67 | Open a clip in the Events dock (a placed timeline opens that timeline; a frozen render opens its notes) | double-click it | `e` on the caret's clip | double-tap | T M D |
| 03.68 | Move a clip along its lane | drag it | — | hold it, then drag | T M D |
| 03.69 | Move a clip to another lane | drag it onto a lane of the same material (lights up) | — | hold it, then drag onto a lane of the same material | T M D |
| 03.70 | Grab, nudge and drop a clip | — | `g` grabs; `h`/← `l`/→ nudge a beat; Enter drops; Esc cancels | — | T D |
| 03.71 | Resize a clip by an edge | drag the strip at its start or end (wider on the selected clip, which shows a grip) | — | hold the edge, then drag | T M D |
| 03.72 | Fade a clip in / out | drag the corner square (whole beats; ⌘/Ctrl or ⇧ frees) | — | hold the corner square, then drag | T M D |
| 03.73 | Cancel a move, resize or fade | release where it started | Esc during a keyboard move | — | T D |
| 03.74 | Pick the tool (arrow, blade, eraser, mute, crossfade) | click the tool button in the floating bar → its menu | `a`, `b`, `d`, `x` (mute has none); Esc back to arrow | tap the tool button | T M D |
| 03.75 | Cut a clip | click it with the blade (refused at head or tail) | ⌘/Ctrl+B at the playhead on the caret's lane | tap with the blade | T M D |
| 03.76 | Erase a clip | click it with the eraser | Backspace / Delete on the caret's clip or the rectangle | tap with the eraser | T M D |
| 03.77 | Mute / unmute a clip | click it with the mute tool | — | tap with the mute tool | T M D |
| 03.78 | Crossfade two clips | click near a clip's end (default half beat); drag sideways to size; Alt-click removes | — | tap (default size only) | T M D |
| 03.79 | Draw a selection rectangle | ⇧-drag from any lane cell | `v` toggles; `h j k l`/arrows grow it; Esc leaves | hold an empty cell, then drag | T M D |
| 03.80 | Copy / paste the selection (clips; notes in the piano roll) | — | `c` or ⌘/Ctrl+C; `p` or ⌘/Ctrl+V at the caret | — | T D |
| 03.81 | Clip menu entries | right-click a clip → Open file; with a rectangle: move, consolidate, to a timeline; Move, Mute, Warp, Pick / remove take, comp seam crossfade, Crossfade, Slice, Gain, Loop, Loop points, Fade in, Fade out, Re-time, Split, Trim silence, Bounce, Remove | — | long-press, or hold and release in place; the clip's ⋮ | T M D |
| 03.82 | Drop an audio or video file on a lane (audio or picture) | drag a file from the OS over the lane (caret shows where) | — | — | D |
| 03.83 | Drop a resource on a lane | drag an audio or video resource from the Resources panel; or the lane's menu → Place <resource> | the lane's menu → Place <resource> | long-press the lane header → Place <resource> | T M D |
| 03.84 | Place a whole timeline on a lane | drag a timeline tab onto a lane | — | — | D |

### Arrange · Navigation and zoom

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.85 | Scroll (also the piano roll and the clip file's waveform strip) | wheel / trackpad slide, scrollbars | — | one-finger drag | T M D |
| 03.86 | Zoom under the pointer (also the piano roll and the waveform strip) | ⌘/Ctrl+wheel (15% a notch); trackpad pinch | — | two-finger pinch | T M D |
| 03.87 | Pan by dragging (also the piano roll, the strip, the clip editor) | middle-button drag | — | — | D |
| 03.88 | Zoom to the rectangle / out / whole song | Zoom into the selection, Zoom out one level, Fit the whole song in Lane views | `z` / `⇧Z` / `0` | tap them in Lane views | T D |
| 03.89 | Switch timeline tab | click Main or a timeline tab | — | tap | T M D |
| 03.90 | New timeline | click "+ timeline" | — | tap | T M D |
| 03.91 | Timeline tab menu | right-click a tab → Play it alone, Rename, Set length, Add a lane, other timeline verbs | — | long-press | T M D |

### Arrange · Second device

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 03.92 | Work the arrangement from a second device | the same timeline with its own caret, zoom and selection; only move, resize, fade a clip and add a track (no tools, reorder, picture lanes or drops) | the same caret and selection keys | pinch to zoom | T M D |

## 04 Arrangement · Video

### Global (every screen)

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 04.01 | Move, resize or hide a floating window | drag its title bar; drag its edges; click ✕ | — | drag the title bar or edges; tap ✕ | T M D |

### Arrange · Picture lanes

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 04.02 | Hide / solo / arm the camera | click M, S, A (A hidden on a shapes lane) | `m` / `s` / `r` on the caret's lane | tap | T M D |
| 04.03 | Move a shot | drag its body | — | hold it, then drag | T M D |
| 04.04 | Trim a shot's head | drag the 8 px left strip (start, in-point and length together) | — | hold the strip, then drag | T M D |
| 04.05 | Trim a shot's tail | drag the 8 px right strip | — | hold the strip, then drag | T M D |
| 04.06 | Cut / erase a shot | click with the blade or the eraser (mute and crossfade do nothing) | ⌘/Ctrl+B at the playhead | tap with the tool | T M D |
| 04.07 | Shot menu entries | right-click a shot → Move to…, Trim the head to…, Set the length…, Silence / hear again, Delete | — | long-press | T M D |
| 04.08 | Picture lane menu entries | right-click its header → Rename, Delete, Move, Add a shot, Hide / Solo, Copy to a timeline, Arm, Open the lane's file | — | long-press | T M D |
| 04.09 | Read a shot's span | hover the shot | — | — | D |

## 05 Graph

### Chain graph and builders · Canvas and cards

The graph canvas (Chains page, Graph and Mix panels, a lane's module row, the builder preview): cards, sockets, cables, knobs, the minimap; the chains list; the module, asset, preset and icon pickers; the chain, module and rack builders.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 05.01 | Pan the canvas | drag empty canvas (4 px slop) | — | one-finger drag | T M D |
| 05.02 | Zoom the canvas | bare wheel or ⌘/Ctrl+wheel zooms about the cursor; trackpad pinch | — | two-finger pinch | T M D |
| 05.03 | Pan with a trackpad | two-finger scroll moves the camera 1:1 | — | — | D |
| 05.04 | Scroll a long tab column in a card | wheel over the column | — | — | D |
| 05.05 | Return to the previous view | double-click empty canvas | — | double-tap | T M D |
| 05.06 | Zoom to a node | double-click a card header | — | double-tap | T M D |
| 05.07 | Select a node | click a card (again deselects) | Enter on a focused card | tap | T M D |
| 05.08 | Add / remove a node from the selection (modules, mixer strips) | ⇧-click | ⇧+Enter | — | T D |
| 05.09 | Box-select modules | ⇧-drag over empty space | — | — | D |
| 05.10 | Move a node | drag its header | — | drag the header | T M D |
| 05.11 | Draw a cable | drag from an output jack to a matching input | — | same | T M D |
| 05.12 | Draw a cable by two clicks | click an output jack, then a target | — | tap, tap | T M D |
| 05.13 | Re-plug a cable | drag from an occupied jack to a new one | — | same | T M D |
| 05.14 | Delete a cable | lift it and release on bare canvas | — | same | T M D |
| 05.15 | Drop a cable in hand | right-click | Esc | long-press | T M D |
| 05.16 | Bind an automation curve to a knob | drag the curve's mauve socket onto a knob | — | same | T M D |
| 05.17 | Wire a note-on trigger to a curve | drag a module's event-out onto the curve's event-in | — | same | T M D |
| 05.18 | Insert a module on a cable | hover the cable, click its `+` | Tab to the `+`, Enter | tap the cable or `+` | T M D |
| 05.19 | Insert a module at a seam | click the seam `+` or the branch button | — | tap | T M D |
| 05.20 | Add a free module | right-click empty canvas → module picker | — | long-press empty canvas | T M D |
| 05.21 | Turn a knob / slider / fader / number | drag vertically (every selected linked node moves too) | — | vertical drag | T M D |
| 05.22 | Fine-adjust a control | hold ⇧ while dragging (×0.25) | — | — | D |
| 05.23 | Type an exact value | double-click a control | Enter commits, Esc cancels | double-tap | T M D |
| 05.24 | Abort a knob drag | release outside the window | — | pointer cancel | T M D |
| 05.25 | Toggle a button, trigger a rack pad | click | Enter | tap | T M D |
| 05.26 | Pick from a dropdown widget | click, choose | native select keys | native picker | T M D |
| 05.27 | Pick a file for a filepicker widget | click the file button → asset tree | — | tap | T M D |
| 05.28 | Switch a group on / off | click the group's dot | — | tap | T M D |
| 05.29 | Play an interactive LCD screen | press, drag, release on it | modifiers reach the script | drag | T M D |
| 05.30 | Bypass / enable a module | click the icon bubble | Enter | tap | T M D |
| 05.31 | Open the plugin's native editor | click the corner bubble | Enter | tap | T M D |
| 05.32 | Open / close an assembly | click the corner bubble | Enter | tap | T M D |
| 05.33 | Switch a card's alternative | click a badge; `+` adds one | — | tap | T M D |
| 05.34 | Switch a card's panel tab | click a tab (or the narrow-card dropdown) | — | tap | T M D |
| 05.35 | Open the preset menu / step presets | click the preset button; `<` `>` step | — | tap | T M D |
| 05.36 | Pick a track's input device | click the input name in the track card | — | tap | T M D |
| 05.37 | Audition the chain at OUT | click Play on OUT | — | tap | T M D |
| 05.38 | Work a rack tap (add, trigger, note, sample, mute, remove) | click `+`, the pad, the chips, mute, ✕ | — | tap | T M D |
| 05.39 | Zoom fit, Auto layout, Stack | click Zoom fit, Auto layout or Stack in the Graph views capsule | — | tap them; on a phone, in Views and panels | T M D |
| 05.40 | Recentre from the minimap | press or drag on the minimap | — | drag | T M D |
| 05.41 | Toggle a lane's curve cards | click the spline button in the row's strip | — | tap | T M D |
| 05.42 | Add a lane input | click an icon in the row's strip | — | tap | T M D |
| 05.43 | Thaw an auto-frozen lane | any edit on the card | — | same | T M D |
| 05.44 | Learn a MIDI controller onto a param | right-click → MIDI learn, turn a hardware knob | hardware | — | T D |

### Chain graph and builders · Graph menus

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 05.45† | Module card menu entries | right-click → node and module verbs, Open file, Download model, Open native UI | — | long-press | T M D |
| 05.46† | Control menu entries | right-click a bound control → MIDI learn, Automate with a curve, Remove automation, Remove MIDI binding | — | long-press | T M D |
| 05.47† | Curve card menu entries | right-click → Edit curve, Edit as automation, Remove automation | — | long-press | T M D |
| 05.48† | Assembly menu entries | right-click a box → assembly verbs, Open template in builder | — | long-press | T M D |
| 05.49† | Rack tap menu entries | right-click a tap → Change note / sample, Rename, Remove, Add before / after, Edit chain | — | long-press | T M D |
| 05.50† | Track card menu entries | right-click → Group the N tracks, Open track file | — | long-press | T M D |
| 05.51† | Group card menu entries | right-click → mute, solo | — | long-press | T M D |

### Chain graph and builders · Chains list

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 05.52 | Open the chain list (phone) | — | — | tap "list" (closes on a pick) | T M |
| 05.53 | Select / expand a chain | click its name (fits the camera; again collapses) | Tab, Enter/Space | tap | T M D |
| 05.54 | Select a module of a chain | click its row (pans to it) | Enter/Space | tap | T M D |

## 06 Source

### Clip editor / Waveform · Waveform

The events panel: the editor toolbar, the audio clip panel (mode strip, header with gain, loop, trim, operations, Done) and the waveform with its marks; the clip file's waveform strip in the editor dock.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 06.01 | Drag a slice or window edge (clip file's strip) | drag the 12 px edge handle (written into the buffer) | — | drag | T M D |
| 06.02 | Audition the strip | click play | — | tap | T M D |
| 06.03 | Close the strip | click "close" / ✕ | — | tap | T M D |

### Source editor and file tree · Source editor

The editor pane (CodeMirror with autosave, vim mode, preview gutter, docked waveform, side pane, reader strips) and the project / libraries file tree.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 06.04 | Edit a project file's text | click to place the caret, drag to select | standard editor keys | on-screen keyboard | T M D |
| 06.05 | Save now | — | ⌘/Ctrl+S (`:w` in vim) | — (autosave) | T D |
| 06.06 | Autocomplete a field or value | click a suggestion | typing opens it; ↑/↓, Enter/Tab accept, Esc closes | tap | T M D |
| 06.07 | Read a lint message | hover the squiggle | — | — | D |
| 06.08 | Open a referenced entity | ⌘/Ctrl+click the underlined name | — | — | D |
| 06.09 | Audition a sample from the gutter | click the play triangle | — | tap | T M D |
| 06.10 | Use vim mode (off by default) | — | normal / insert / visual / ex keys; one change is one undo | hardware keyboard only | T D |
| 06.11 | Open the module in the Builder | click "Builder" (module files) | — | tap | T M D |
| 06.12 | Edit the module behind a node beside it | type in the side pane | same keys | same | T M D |
| 06.13 | Set a picture reader's in / out frame | use the readers strip | — | tap / drag | T M D |
| 06.14 | Show the file tree (phone) | — | — | tap "files" (closes on a pick) | T M |
| 06.15 | Follow a link in rendered Markdown | click it | — | tap | T M D |

### Source editor and file tree · File tree

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 06.16 | Open a file (or a module, chain, assembly in its builder) | click the row | Tab, Enter | tap | T M D |
| 06.17 | Open a project from the workspace level | click its row | Enter | tap | T M D |
| 06.18 | Switch Project / Libraries | click a tab | — | tap | T M D |
| 06.19 | Pin / unpin a file, open a pinned file | menu Pin / Unpin; click a pinned row; its ✕ | Enter | tap; long-press for the menu | T M D |
| 06.20 | Row menu entries | right-click a row or click ⋯ (on hover) → Upload files / folder, New file / folder / module / node, Clone module, Pin, Copy path, Rename, Delete | — | long-press or tap ⋯ | T M D |
| 06.21 | Background menu entries | right-click empty space → root actions | — | long-press empty space | T M D |
| 06.22 | Name a new, renamed or cloned item | type in the inline prompt | Enter commits, Esc closes | on-screen keyboard | T M D |
| 06.23 | Delete an item | menu Delete, confirm | — | same | T M D |
| 06.24 | Move a file or folder into a folder | drag a row onto a folder | — | — | D |
| 06.25 | Upload by dropping | drop OS files or folders on a folder row or the background | — | from Files on iPad; none on phone | T D |
| 06.26 | Upload from a picker | menu "Upload files / folder" | — | native chooser | T M D |

## 07 Resources panel

### Arrange · Reference player and stems bar

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 07.01 | Add reference tracks | click "+" in References → file picker | — | tap | T M D |
| 07.02 | Play / pause a reference | click its row (again pauses) | — | tap | T M D |
| 07.03 | Remove a reference | click the row's trash icon | — | tap | T M D |
| 07.04 | Play / pause the reference bar | click play in the mini player | — | tap | T M D |
| 07.05 | Seek in the reference | click or drag the slider | arrows (written on key up) | drag | T M D |
| 07.06 | Close the reference bar | click ✕ | — | tap | T M D |

### Resources

The imported clips list.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 07.07 | Preview an imported clip | click play / stop on the row | — | tap | T M D |

## 08 Tasks & history panel

### Global (every screen)

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 08.01 | Show running background processes | click the processes button (read-only popover) | Enter/Space on the button | tap | T M D |

### Tasks & history

The task list with project tags, the edit journal, the Saves list, the sync history.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 08.02 | Add a task | type in the line, click add | Enter | tap | T M D |
| 08.03 | Pin a new task to the playhead | tick the checkbox beside the line | Space | tap | T M D |
| 08.04 | Tick / untick a task | click its checkbox | Space | tap | T M D |
| 08.05 | Delete a task | click the bin (on hover; always on touch), confirm | Enter | tap | T M D |
| 08.06 | Seek to a task's timecode | click the timecode (same project) | — | tap | T M D |
| 08.07 | Add a project tag | type, Enter | Enter | on-screen keyboard | T M D |
| 08.08 | Remove a project tag | click ✕ on the chip | — | tap | T M D |
| 08.09 | Light what an undo step touched | hover a journal row | — | — | D |
| 08.10 | Switch Edits / Workspace / Saves | click a toggle (Saves only when the workspace is under git) | — | tap | T M D |

## 09 Export panel

### Export

The share panel: timeline choice, audio exports, video export row.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 09.01 | Choose the timeline to render | pick in the select | native | native | T M D |
| 09.02 | Export MP3 / WAV | click the button | — | tap | T M D |
| 09.03 | Export each track (zip) | click the button | — | tap | T M D |
| 09.04 | Choose the video format | click MP4 / WebM | — | tap | T M D |
| 09.05 | Choose the video size | click 720p / 1080p | — | tap | T M D |
| 09.06 | Export video | click Download | — | tap | T M D |

## 10 Settings panel

### Settings · In-project settings panel

The `/settings` page (General, Devices, Services, Downloads, About), the engine page (Engine, Instruments, Chains, Plugins; a phone variant), the in-project Settings panel (Editor, I/O, Project) and the full-screen command editor.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 10.01 | Switch tab (Editor / I/O / Project) | click the tab | `ui settings <tab>` in the bar | tap | T M D |
| 10.02 | Close the settings panel | click ✕ | Esc | tap ✕ | T M D |
| 10.03 | Open the Commands page | click Commands and shortcuts in the Editor tab | `ui settings commands` | tap | T M D |
| 10.04 | Toggle vim mode (Editor tab) | click its switch (off by default) | `ui vim` | tap | T M D |
| 10.05 | Choose the summing console (Project tab) | pick in the select | native select keys | tap | T M D |

## 11 Clip editor

### Clip editor / Waveform · Clip editor shell

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 11.01 | Run a process chain over a clip file (Process menu) | click Process, search, click a chain | arrows, Enter | tap | T M D |
| 11.02† | Keep mine or take theirs after a disk conflict | click "keep mine" / "take theirs" | — | tap | T M D |

### Clip editor / Waveform · Audio clip panel

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 11.03 | Switch the waveform view (standard / onsets / warp / takes) | pick in the view select, or click a mode strip button | `a` standard, `o` onsets, `w` warp; Esc back to standard; takes only via `ui wave takes` | tap a mode strip button | T M D |
| 11.04 | Zoom the bar window | bare wheel zooms about the cursor; ⌘/Ctrl+wheel or trackpad pinch zooms smoothly | — | two-finger pinch | T M D |
| 11.07 | Set the event's gain | click "gain: 0.0 dB" in the title bar, drag the knob in the bubble above it (written on release) | Tab, Enter opens the bubble; arrows on the knob (written on key up) | tap gain, drag the knob | T M D |
| 11.08 | Reset the gain to 0 dB | double-click the knob in the gain bubble | — | double-tap the knob | T M D |
| 11.09 | Toggle the event's loop (also in the roll header) | click "loop: off" in the title bar, then the switch in its bubble (the roll header keeps its Loop button) | Tab, Enter opens the bubble; Space toggles the switch | tap loop, tap the switch | T M D |
| 11.10 | Move the loop's start or end | drag a brace handle on the bars ruler (free) | — | drag | T M D |
| 11.11 | Set the onset sensitivity (onsets view) | drag the threshold slider | arrows | drag | T M D |
| 11.12 | Open the trim gate | click the lightning Operations button, then Trim: the gate opens as a popover from the button (the kept part is drawn over the wave) | Tab, Enter | tap the lightning button, tap Trim | T M D |
| 11.13 | Set the trim gate threshold | drag the dB slider in the Trim popover | arrows | drag | T M D |
| 11.14 | Trim the event to the gate | click Trim in the Trim popover | Tab, Enter | tap | T M D |
| 11.15 | Run an operation chain on the clip | click the lightning Operations button, Transformations, search, click a chain | arrows, Enter | tap the lightning button, Transformations, tap a chain | T M D |
| 11.16 | Discard the take | click the trash button | — | tap | T M D |
| 11.17 | Finish editing | click Done (files the take as reviewed) | — | tap | T M D |
| 11.18 | Close the editor | click Close editor in the dock header | Esc at the end of the ladder | tap | T M D |
| 11.29† | Centre the timeline on the clip being edited | click Fit, the first button of the title bar: the clip centred, its lane on screen, the clip outlined 1.5 s; Fit again goes back to the view before (so does ⇧Z) | `ui fit` in the command bar | tap Fit | T M D |

### Clip editor / Waveform · Waveform

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 11.19 | Place a stretch point (warp view) | double-click the wave (not snapped) | — | double-tap | T M D |
| 11.20 | Move a stretch point | drag the blue handle (written on release) | — | drag | T M D |
| 11.21 | Remove a stretch point | right-click the handle → "clip warp remove" | — | long-press | T M D |
| 11.22 | Add an onset (onsets view) | click inside the event (snaps to the grid) | — | tap | T M D |
| 11.23 | Remove an onset | click its pin (under 4 px of travel) | — | tap the pin | T M D |
| 11.24 | Move an onset | drag its pin (refused outside the event) | — | drag | T M D |
| 11.25 | Trim the event's window | drag an edge handle of the lit band | — | drag | T M D |
| 11.26 | Edit a comped part's edges (takes view) | drag the band's edges | — | drag | T M D |
| 11.27 | Draw a selection band | drag past 4 px (nothing written) | — | drag | T M D |
| 11.28 | Add an onset or a stretch point by value (List tool) | type the time in Add at (seconds), click Add onset or Add stretch point | Enter adds | tap, type, tap | T M D |

## 12 Piano roll

### Clip editor / Waveform · Clip editor shell

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 12.01 | Switch the view of a MIDI clip | click the view dropdown, pick an entry | arrows, Enter in the menu | tap, tap | T M D |
| 12.02 | Close the view dropdown without picking | click outside | Esc | tap outside | T M D |
| 12.03 | Open a sequence file as Text or Roll | click the Text / Roll tab in the toolbar | — | tap | T M D |

### Piano roll · Lanes

The roll: mode strip (select, create, remove, takes), key gutter, note lanes with grid caret, loop braces, song ruler; notes with velocity line, edges and body. Also the simple roll of a sequence file.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 12.04 | Pick a roll mode | click a tool capsule in the title bar (the waveform's views drawn the same way) | `a` select, `n` create, `d` remove; Esc back to select; takes only via `ui roll takes` | tap a tool capsule | T M D |
| 12.05 | Draw a note (create mode) | press at the pitch, drag right, release (a click is one cell; refused outside the event's window) | — | press and drag; tap for one cell | T M D |
| 12.06 | Draw off the grid | hold ⌘/Ctrl while drawing | — | — | D |
| 12.07 | Frame a rectangle of cells | drag on a bare lane (⇧-drag in create mode) | `v` toggles; arrows or `h j k l` grow it; Esc or `v` cancels | drag | T M D |
| 12.08 | Move the roll's caret | click the lane | arrows or `h j k l` | tap | T M D |
| 12.09 | Select a whole pitch row | double-click its key in the gutter | — | double-tap | T M D |
| 12.10 | Delete the notes in the rectangle or under the caret | — | Backspace, Delete or `x` | — | T D |
| 12.11 | See the row and column under the pointer | hover the lane | — | — | D |
| 12.12 | Move the loop's start or end | drag a brace handle (snaps to the grid; Alt frees) | — | drag | T M D |
| 12.13 | Audition a pitch | click a key in the gutter | — | tap | T M D |
| 12.14 | Hear a note while editing | automatic on a press, a note, and each row of a transpose drag | — | same | T M D |
| 12.15 | Multiply the sequence's length by 2 | click the lightning Operations button, then "Multiply the length by 2" | — | tap the lightning button, tap the entry | T M D |

### Piano roll · Notes

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 12.16 | Move a note in time and pitch | drag the body (time snaps; rows transpose) | — | drag the body | T M D |
| 12.17 | Move a note by whole grid steps | hold ⌘/Ctrl while dragging | — | — | D |
| 12.18 | Move a note with no snap | hold ⇧ while dragging | — | — | D |
| 12.19 | Stretch a note's start / end | drag the left / right edge | — | drag | T M D |
| 12.20 | Set a note's velocity | drag the white line up or down | — | drag the line | T M D |
| 12.21 | Move the whole selection | drag one note's body in the rectangle (refused if blocked) | — | drag | T M D |
| 12.22 | Remove a note (remove mode) | click the note | `x` / Backspace on the caret | tap | T M D |
| 12.23 | Draw over a note (create mode) | press on a note: a new note starts | — | same | T M D |
| 12.24 | Note menu entries | right-click a note → remove, split at the caret (caret inside), and for the rectangle's notes remove / transpose / quantize / velocity | — | long-press | T M D |

### Piano roll · Sequence file roll

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 12.25 | Add or remove a note | click an empty cell / a note (1/16 grid) | — | tap | T M D |

### Piano roll

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 12.26† | Centre the timeline on the clip being edited | click Fit, the first button of the title bar: the clip centred, its lane on screen, the clip outlined 1.5 s; Fit again goes back to the view before (so does ⇧Z) | `ui fit` in the command bar | tap Fit | T M D |

## 13 Node chain editor

### Chain graph and builders · Pickers

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 13.01 | Browse the picker tree | hover a branch opens it; click a leaf | focus a branch opens it | tap a branch, tap a leaf | T M D |
| 13.02 | Search a picker | type in the field | ↑/↓ move, Enter picks | on-screen keyboard | T M D |
| 13.03 | Back out of a picker | click outside, scroll, blur | Esc clears the search, then closes | tap outside | T M D |
| 13.04 | Pick "none" in an asset picker | click "- none -" | — | tap | T M D |
| 13.05 | Save a preset under a name | type, choose scope, click Save | Enter | same | T M D |
| 13.06 | Overwrite the loaded preset | click "Update <name>" | — | tap | T M D |
| 13.07 | Pick the engine a module runs as | click an engine in the side list | — | tap | T M D |
| 13.08 | Apply a preset | click a preset | Enter after search | tap | T M D |
| 13.09 | Pick an icon | type to search, click an icon or "none" | — | tap | T M D |

### Chain graph and builders · Canvas and cards

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 13.10 | Open a module card's menu | click ⋯ on the card | Enter on ⋯ | tap ⋯ | T M D |

## 14 Export · folder picker

No interaction of its own: its controls are listed under 09 and 01.

## 15 Export · saved

No interaction of its own: its controls are listed under 09 and 01.

## 16 Pull in place

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 16.01 | Open the merge | click "N conflicts" (opens by itself when a pull diverges) | — | tap | T M D |

## 17 Takes mode

### Takes

The takes view in the audio and roll panels: take rows, record button, span, crossfade tool, comp strip.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 17.01 | Record / stop another take | click the red record button (punches into the span, else from the event's start); click again to stop | — | tap | T M D |
| 17.02 | Draw a span across the takes (audio) | drag on a take row (snaps to 1/4 beat) | — | drag | T M D |
| 17.03 | Play a take over the span | click a row once a span exists | — | tap | T M D |
| 17.04 | Clear the span | click ✕ beside the span read-out | Esc | tap | T M D |
| 17.05 | Seek the song from a row | click a row with no span (not snapped) | — | tap | T M D |
| 17.06 | Use a whole take (or its span) | click the check on its label | — | tap | T M D |
| 17.07 | Pick a MIDI take whole | click its row | — | tap | T M D |
| 17.08 | Remove a take | click the trash on its label | — | tap | T M D |
| 17.09 | Turn the crossfade tool on / off (audio) | click the crossfade button | — | tap | T M D |
| 17.10 | Size a crossfade at a seam | with the tool on, drag sideways near a seam (1/8 beat) | — | drag | T M D |
| 17.11 | Remove a crossfade at a seam | Alt-press at the seam | — | — | D |
| 17.12 | Select a comped part to edit | click it in the comp strip | Tab, Enter | tap | T M D |

## 18 Command palette

### Command bar

The floating command line with its suggestion list and parameter fields.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 18.01 | Type a command / filter the list | click into the field, type | type; ↓ opens the list, ↑/↓ walk it | tap, type | T M D |
| 18.02 | Pick a command | click a row | Enter or Tab on the highlighted row (runs the typed line if nothing matches) | tap a row | T M D |
| 18.03 | See why a row is offered | hover a row | — | — | D |
| 18.04 | Dismiss the bar | click outside (stays while a command waits) | Esc; running a command closes it | tap outside | T M D |

## 19 Command palette · argument

### Command bar

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 19.01 | Fill a parameter | click a field; click a suggestion | Tab takes the suggestion and moves on; Enter picks / next / runs; ↑/↓ walk | tap | T M D |
| 19.02 | Abandon a half-filled command | click the command name | Esc closes suggestions, a second Esc abandons | tap the name | T M D |
| 19.03 | Answer a parameter with a gesture in a pane | click or drag in a pane while the bar waits (e.g. click the ruler for a beat; live preview) | — | tap or drag in the pane | T M D |
| 19.04 | Pre-write a command from a menu | pick a context-menu row that still owes an argument | — | tap the row | T M D |

## 20 Turntables

### Turntables

The DJ desk: two decks (vinyl, waveform, BPM fader, hot cues, loop row, censor, echo), the mixer column (knobs, channel faders, crossfader), the song browser.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 20.01 | Load a song onto a deck | click the record label (or label button) → browser | type to search; Enter loads the first hit | tap | T M D |
| 20.02 | Scratch / hold the record | press the disc and rotate (±4×) | — | finger rotation | T M D |
| 20.03 | Start / stop a deck | click Play | — | tap | T M D |
| 20.04 | Shuttle on the waveform | press and pull sideways (farther = faster, 16×); release seeks | — | drag | T M D |
| 20.05 | Set the deck tempo | drag the BPM fader (written on release) | arrows (written on key up) | drag | T M D |
| 20.06 | Reset the tempo to the record's | double-click the BPM fader | — | double-tap | T M D |
| 20.07 | Halve / double the detected tempo | click ÷2 / ×2 | — | tap | T M D |
| 20.08 | Toggle key lock | click KEY | — | tap | T M D |
| 20.09 | Sync a deck to the other | click Sync | — | tap | T M D |
| 20.10 | Jump to a hot cue | short press (< 1 s) a cue button | — | tap | T M D |
| 20.11 | Set a hot cue | press and hold 1 s | — | hold 1 s | T M D |
| 20.12 | Censor (play backwards while held) | hold the censor button | — | hold | T M D |
| 20.13 | Echo out (held) | hold the echo-out button | — | hold | T M D |
| 20.14 | Headphone cue | click the headphone button | — | tap | T M D |
| 20.15 | Step the loop window | click `<` / `>` | — | tap | T M D |
| 20.16 | Set loop IN / OUT | click IN / OUT | — | tap | T M D |
| 20.17 | Start a beat-length loop | click a length (1, 2, 4…) | — | tap | T M D |
| 20.18 | Halve / double the loop | click ÷2 / ×2 | — | tap | T M D |
| 20.19 | Loop roll (held) | hold the roll button | — | hold | T M D |
| 20.20 | Set trim / EQ / filter / echo / stem level | drag a knob vertically (160 px full travel) | arrows step 1/80 | vertical drag | T M D |
| 20.21 | Reset a knob to neutral | double-click it | Home | double-tap | T M D |
| 20.22 | Move a channel fader | drag it | arrows | drag | T M D |
| 20.23 | Reset a channel fader to 0 dB | double-click it | — | double-tap | T M D |
| 20.24 | Move the crossfader | drag it | arrows | drag | T M D |
| 20.25 | Re-centre the crossfader | double-click it or its title | — | double-tap | T M D |
| 20.26 | Toggle quantise | click Quantise | — | tap | T M D |
| 20.27 | Refresh the set list | click refresh | — | tap | T M D |

## 21 Library

### Home and its pages · Frozen chains

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 22.01 | Create a frozen chain on the bench | click "New" | Enter | tap | T M D |
| 22.02 | Edit a chain on the bench | click the pencil (disabled when it cannot run live) | Enter | tap | T M D |
| 22.03 | Duplicate a chain | click the copy icon → name question | Enter / Esc | tap | T M D |
| 22.04 | Delete a chain | click the trash icon → confirm | Enter / Esc | tap | T M D |
| 22.05 | Save a chain to this machine's or another engine's slot | click the chip icon, or pick a target in its menu | Enter, arrows | tap | T M D |

### Project header and transport

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 22.06† | Save a bench chain | click "Save to chain" (bench only) | — | tap | T M D |

## 22 Frozen chains (not drawn)

No interaction of its own: its controls are listed under .

## 23 App settings

### Settings · Settings page and engine page

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 01.13 | Clear finished downloads | Settings → Downloads → Clear finished | — | tap | T M D |
| 23.02 | Switch tab | click the tab: General, Devices, Services, Downloads, About (kept in `?tab=`) | ←/→ | tap | T M D |
| 23.03 | Change the language | click a language button | Enter | tap | T M D |
| 23.04 | Save the Claude API key | type in the password field, click Save | Enter saves | tap, type | T M D |
| 23.05 | Remove the Claude API key | click Remove | — | tap | T M D |
| 23.06 | Change the AI model | type, click Save | Enter saves | same | T M D |
| 23.07 | Revoke a paired device | click Revoke | — | tap | T M D |
| 23.08 | Choose the separation chains | pick in the select | ↑/↓, Enter | tap, pick | T M D |
| 23.09 | Download a missing model | click the link under the select | — | tap | T M D |
| 23.10 | Choose the primary audio interface | click Primary → pick an interface | Enter, arrows | tap, pick | T M D |
| 23.11 | Choose the fallback audio interface | click Fallback → pick an interface or None (the primary is not offered) | Enter, arrows | tap, pick | T M D |
| 23.12 | Download, cancel, remove or retry an instrument | click the button on its row (Downloads tab) | Enter | tap | T M D |
| 23.13 | Open the Engine page | Devices → click the Engine row | Enter | tap | T M D |
| 23.14 | Read k7's licence, open its source, read a third-party notice | About → Read it; Open; search, click a notice | Enter | tap | T M D |
| 23.15 | Sign out of GitHub | Services → GitHub (shown once signed in): click "Sign out" beside @login; the key k7 added stays on GitHub | Tab, Enter | tap | T M D |
| 23.16 | Manage this device's keys on GitHub | click "Manage keys on GitHub" (github.com/settings/keys, a new tab) | Tab, Enter | tap | T M D |
| 23.17 | Add a Forgejo server | Services → Forgejo servers: click "Add a Forgejo server" → the server's address and Check | Tab, Enter | tap | T M D |
| 23.18 | Check the server's address | type the address (`git.example.com`), click Check → "Forgejo <version> answered.", "Large files (LFS) are on." or the warning that audio files will not sync, and whether the server's web address answers; then Add server, nothing to register | Enter checks | tap, type | T M D |
| 23.19 | Copy a redirect URI | only when the server's admin turned off its built-in sign-in apps: open "The server's admin turned off its built-in sign-in apps", then click Copy beside each of the two addresses, to paste as the redirect URIs of an OAuth2 application made on the server (Settings → Applications → OAuth2 applications, "Confidential client" unticked) | — | tap Copy | T M D |
| 23.20 | Enter the Client ID | in the same disclosure, paste the application's Client ID in "Client ID" | — | tap, type | T M D |
| 23.21 | Give the server's SSH host and port | open "SSH host and port, if different", type SSH host and SSH port | Enter on the disclosure | tap, type | T M D |
| 23.22 | Add the server | click "Add server" (on once the address is checked) → the server's section, Not signed in | — | tap | T M D |
| 23.23 | Cancel adding a server | click Cancel under the form | — | tap | T M D |
| 23.24 | Remove a Forgejo server | in the server's section click Remove → "Remove <server> from this device?" in place → "Remove server", or Cancel | Tab, Enter | tap | T M D |

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 23.01 | Save the commit identity (Settings) | type name and email; they save themselves | — | tap | T M D |

## 24 Logs panel

### Logs

The log list.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 24.01 | Read the log | scroll the list | — | drag to scroll | T M D |
| 24.02 | Clear the log | click "clear" | — | tap | T M D |

## 25 Playlists panel

### Arrange · Reference player and stems bar

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 25.01 | Start stem separation | click "Start" (disabled while running or with no chain) | — | tap | T M D |
| 25.02 | Open settings from the stems bar | click Settings (no chain set) | — | tap | T M D |
| 25.03 | Close the stems bar | click ✕ | — | tap | T M D |

### Playlists

The set list picker, the playlist rows, the library search, the stems bar, the playlist text editor.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 25.04 | Choose a set list | click the title, search and pick | picker keys | tap | T M D |
| 25.05 | Create a set list | click "+", type, Create | Enter, Esc | tap | T M D |
| 25.06 | Open the playlist as text | click maximize | — | tap | T M D |
| 25.07 | Back from the playlist text | click Back | — | tap | T M D |
| 25.08 | Search the library to add a record | click "Search", type | Enter adds the first hit, Esc closes | tap, type | T M D |
| 25.09 | Add a record | click a hit | — | tap | T M D |
| 25.10 | Play / pause a row | click its play button (the next starts at the end) | native button keys | tap | T M D |
| 25.11 | Move a row up / down, remove it | click its arrows or bin | — | tap | T M D |
| 25.12 | Drag a record onto a turntables deck | drag a playable row onto any part of a deck | — | — | D |
| 25.13 | Open the stems bar for the whole set | click "Separate" | — | tap | T M D |

## 26 Command editor screen

### Settings · Commands page

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 26.01 | Filter commands by text | type in the filter field | typing | tap, type | T M D |
| 26.02 | Filter by entity | click an entity pill | — | tap | T M D |
| 26.03 | Pick a command's icon | click the icon square → icon grid | — | tap | T M D |
| 26.04 | Rename a command (per language) | type in its language field | typing | tap, type | T M D |
| 26.05 | Choose which right-click menus offer a command | click the entity button, tick entities, "give back" | — | tap | T M D |
| 26.06 | Bind a shortcut | click the key button, press the key | the key; Esc gives up, Backspace/Delete removes | — | T D |
| 26.07 | Reorder a command | click ↑ / ↓ | — | tap | T M D |
| 26.08 | Give a command back its defaults | click reset | — | tap | T M D |
| 26.09 | Save / cancel the draft | click Save / Cancel | ⌘/Ctrl+S saves | tap | T M D |
| 26.10 | Leave the page | click Back → discard question when unsaved | Enter / Esc | tap | T M D |

## 27 Mixer panel

### Chain graph and builders · Canvas and cards

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 27.01 | Walk along a row graph (Mixer, module row) | wheel scrolls sideways | — | horizontal drag | T M D |

### Mixer

The Mixer (one strip per lane's mixer module, Master as the row's last strip with its fader, pan read as L / C / R), the mastering controls under OUT, the I/O tab's meters.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 27.02 | Set a strip's fader, pan, mute or solo | drag the control; click a toggle | Enter on a toggle | vertical drag / tap | T M D |
| 27.03 | Move a mastering band slider | drag it (written on release) | arrows (written on key up or blur) | drag | T M D |
| 27.04 | Choose a mastering option | pick in the select | native | native | T M D |

## 28 Events panel

### Clip editor / Waveform · Audio clip panel

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 28.01 | Stop a rolling take (audio or MIDI) | click Stop in the header | Esc while an audio take rolls | tap | T M D |

## 29 Sync settings

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 29.01 | Enter the ssh address | Other host tab: type the address; an https://, http:// or git:// address reads "k7 syncs over SSH only" and cannot be sent | — | tap, type | T M D |
| 29.03 | Copy this device's public key | Other host tab: click Copy (the box selects all on focus) | — | tap Copy | T M D |
| 29.04 | Connect / Clone | Other host tab: click Connect (Sync settings) or Clone (Clone a repository); "Contacting the server…" / "Fetching the repository…" while it runs | Enter in the address field | tap | T M D |
| 29.05 | Leave Sync settings or Clone | click "‹ Projects" (Back to Projects) at the top of the page | Esc | tap | T M D |
| 29.06† | Go to Settings → Services from the GitHub tab | GitHub tab with no account that lists repositories: click "Open Settings → Services" and sign in there (41.01) | Tab, Enter | tap | T M D |
| 41.02 | Choose a service tab | click GitHub, Forgejo or Other host (the page opens on the first service that can list repositories; on Other host with no such account, or with an address to show) | arrows in the tab list, Tab | tap | T M D |

## 30 Workspace history

### Tasks & history

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 30.01 | Expand a checkpoint or edit | click the row | — | tap | T M D |
| 30.02 | Restore the workspace to a point | click the restore icon (on hover; always on touch), confirm | — | tap | T M D |
| 30.03 | Name a checkpoint | click "Name", answer | Enter | tap | T M D |

## 31 Conflict

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 31.01 | Choose mine / theirs / both for a conflict | click a choice button | Tab, Enter/Space | tap | T M D |
| 31.02 | Take all mine / all theirs | click "all mine" / "all theirs" | — | tap | T M D |
| 31.03 | Finish the merge | click Finish (once every conflict is chosen) | — | tap | T M D |
| 31.04 | Abort the merge | click Abort → red Abort (or Keep) | Esc closes | tap | T M D |
| 31.05 | Decide the merge later | click Later or the backdrop | Esc | tap | T M D |

## 32 Host key prompt

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 32.01 | Trust a new host key | click "Trust" under the fingerprint | — | tap | T M D |
| 32.02 | Forget a changed host key | click "Forget" in the red warning | — | tap | T M D |

## 33 Saves list

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 33.01 | Load older saves | click "Older saves" at the foot of the Saves list | — | tap | T M D |
| 33.02 | Open the Saves list from the git menu | the header's git capsule: ⌄ ("More git actions") → "Saves" → the History panel on its Saves list | Enter, arrows in the menu | phone: tap the git button → Saves | T M D |

## 34 Save

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 34.04 | Save from the project header | click Save in the header's git capsule ("Save N changes"; "Save: nothing to save", dimmed, when nothing waits; a spinner while it runs) | Tab, Enter | phone: tap the git button → Save | T M D |
| 34.05 | Pull after a save not sent | toast "Saved on this device, not sent" · "The remote has new changes: pull first, then save again.": click Pull | — | tap | T M D |
| 34.06 | Retry a save | toast "Not saved", or "Saved on this device, not sent" for any failure but a remote ahead: click Retry | — | tap | T M D |
| 34.07 | Set the author after a save refused | toast "Not saved: no author yet" · "Set your name and email in Settings, then save again.": click Settings → Settings, General (23.01) | — | tap | T M D |

## 35 Workspace tree (not drawn)

### Home and its pages · Workspace tree

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 35.01 | Go back to the projects list | click "← Projects" | Enter | tap | T M D |
| 35.02 | Unfold / fold a folder | click the folder row or its chevron | Enter/Space | tap | T M D |
| 35.03 | Read a file (read-only viewer) | click the file row | Enter/Space | tap | T M D |
| 35.04 | Open a project folder in k7 | click "Open in k7" on its row | Enter | tap | T M D |

## 36 Engine page (not drawn)

### Settings · Settings page and engine page

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 36.01 | Start / restart / stop the engine | click Start, Restart or Stop | — | tap | T M D |
| 36.02 | Prefer an audio interface | click the up/down arrows on its row; "Use" on the ASIO hint | — | tap | T M D |
| 36.03 | Open the engine to the LAN | click the switch (desktop and phone) | Space on the switch | tap | T M D |
| 36.04 | Enable / disable a saved slot | click Enable or Disable | — | tap | T M D |
| 36.05 | Set a slot's footswitch CC | click Footswitch → number question (0–127, empty = none) | Enter / Esc | tap | T M D |
| 36.06 | Remove a slot | click Remove | — | tap | T M D |
| 36.07 | Download / remove an instrument | click the row's action button | — | tap | T M D |
| 36.08 | Rescan plugins | click Rescan (disabled while stopped) | — | tap | T M D |
| 36.09 | Quit the phone engine | click Quit | — | tap | T M D |
| 36.10 | Toggle full screen (Android) | click the switch | — | tap | T M D |
| 36.11 | Trust a private CA / forget the added CAs (phone) | click "Trust" / "Forget" | — | tap | T M D |

## 37 Help (not drawn)

### Help

The documentation popin: a table of contents on the left (a Contents button on narrow screens), the pages on the right.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 37.01 | Open Help | Home: page ⋮ → Help; a phone: the Help tile in Views and panels; anywhere: `ui help` in the command bar | `?` | tap Help in the projects ⋮ menu or in Views and panels | T M D |
| 37.02 | Jump to a documentation section | click an entry in the table of contents | Tab, Enter; ⌘/Ctrl+F searches the page | tap Contents (narrow screens), then an entry | T M D |
| 37.03 | Close Help | click Back | Esc | tap Back | T M D |

## 38 Curves (not drawn)

### Curves

The curve editor dialog: name, shape, sliders, ADSR knobs, run conditions, a drawable plot.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 38.01 | Name a new curve | type in the name field | typing | on-screen keyboard | T M D |
| 38.02 | Choose the shape | pick in the shape select | arrows | tap, pick | T M D |
| 38.03 | Set amplitude and phase | drag the sliders | arrows | drag | T M D |
| 38.04 | Set polarity | click a radio | arrows | tap | T M D |
| 38.05 | Set the cut | pick in the cut select | arrows | tap, pick | T M D |
| 38.06 | Set cycles per bar and duration | type in the fields | typing, Up/Down | on-screen keyboard | T M D |
| 38.07 | Turn an ADSR knob | drag up/down; ⇧ for a tenth of the speed | Up/Right more, Down/Left less; ⇧ fine | drag vertically | T M D |
| 38.08 | Add a run condition | click "add condition" | — | tap | T M D |
| 38.09 | Edit a condition | pick, type, tick | typing | tap | T M D |
| 38.10 | Remove a condition | click its remove button | — | tap | T M D |
| 38.11 | Add a point (custom, on/off) | click the plot | — | tap | T M D |
| 38.12 | Move a point | drag it | — | drag | T M D |
| 38.13 | Remove a point | double-click it (the last one stays) | — | double-tap | T M D |
| 38.14 | Save the curve | click Create / Save | Tab, Enter | tap | T M D |
| 38.15 | Cancel the curve | click Cancel or outside | Esc | tap | T M D |

## 39 Chain and module builders (not drawn)

### Chain graph and builders · Builders

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 39.01 | Switch the open chain file | click a file chip | — | tap | T M D |
| 39.02 | Add a node to a folder chain | click "+ node", answer the prompt | Enter | tap | T M D |
| 39.03 | Edit the chain text | type in the YAML pane (autosave) | ⌘/Ctrl+S flushes | — | T D |
| 39.04 | Close the chain builder | click close | — | tap | T M D |
| 39.05 | Dismiss the builder's error strip | click it | — | tap | T M D |
| 39.06 | Add a widget | click a type in the Add palette | — | tap | T M D |
| 39.07 | Select a widget / clear the selection | press a widget; press the background | — | tap | T M D |
| 39.08 | Move a widget | drag it (a group carries its content) | — | drag | T M D |
| 39.09 | Resize a widget | drag a corner handle | — | drag | T M D |
| 39.10 | Grab the group around a widget | double-click a framed widget | — | double-tap | T M D |
| 39.11 | Bind a parameter to a widget | drag a parameter row onto a widget, an empty cell or the inspector | — | — | D |
| 39.12 | Cut / copy / paste a widget | right-click a widget or cell → grid menu | Esc closes | long-press | T M D |
| 39.13 | Delete a widget | click Delete in the inspector | — | tap | T M D |
| 39.14 | Auto layout / AI organize | click the palette buttons | — | tap | T M D |
| 39.15 | Rename / reorder / remove / add a tab | name field, up/down/remove icons, add button | Enter commits, Esc cancels | tap | T M D |
| 39.16 | Switch builder pane (Parameters / grid / text) | click a pane button | — | tap | T M D |
| 39.17 | Save the module | click Save | ⌘/Ctrl+S | tap | T M D |
| 39.18 | Scan / native UI / snapshot base | click the Parameters pane buttons | — | tap | T M D |
| 39.19 | Choose input channels | click a count on the channel bar (also under an input's text) | — | tap | T M D |
| 39.20 | Pick a shape / seed | click the shape bar buttons | — | tap | T M D |
| 39.21 | Step a number | click `-` / `+` | — | tap | T M D |
| 39.22 | Put a sample on a rack pad | drag audio files from the OS onto a pad | — | drag from Files on iPad; none on phone | T D |
| 39.23 | Audition / clear a pad | click Play/Pause or ✕ | — | tap | T M D |
| 39.24 | Name a pad or the kit, choose the destination | type; pick project or `@library` | native | native | T M D |
| 39.25 | Create the rack | click "Create" (disabled until valid) | — | tap | T M D |
| 39.26 | Reorder, remove, add a tap effect | up / down / remove icons; "add" opens the picker | — | tap | T M D |
| 39.27 | Close a builder popin | click ✕ or the backdrop | — | tap | T M D |

## 40 Surfaces

### Surfaces

The Surfaces tab lists, by device, the surfaces of the projects open on the network; a surface opened on another device draws alone, with a round Home button, a command line and the transport; the device that holds the project dims the pane a remote shows; pairing asks the engine's machine.

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 40.01 | Run a command from a lone surface | type in the line | Enter runs | on-screen keyboard | T M D |
| 40.02 | Toggle fullscreen | click the footer button | — | tap | T M D |
| 40.03 | Back to the surface list | click the round Home button (another device's surface) or Surfaces in the foot (a surface on its own machine) | — | tap | T M D |
| 40.04 | Open a surface from the index | click its row on the Surfaces tab (a row that cannot open stays greyed, with its reason) | Enter | tap | T M D |
| 40.05 | Refresh the surface list | click Refresh on the Surfaces tab (it also asks again every 5 s) | — | tap | T M D |
| 40.06† | Ask to pair this device | click "Pair" (the code shows large) | — | tap | T M D |
| 40.07 | Play, stop and read the position on another device's surface | click Play or Stop in the transport capsule; click the position to switch bars and seconds | Space plays and stops | tap | T M D |
| 40.08 | Open a pane another device shows (the device that holds the project) | click the dimmed pane: a toast says “In use on <device>” | its pane key says the same | tap it | T M D |

## 41 Forge sign-in

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 41.01 | Sign in with GitHub | Settings → Services → GitHub: click "Sign in with GitHub" (shown while not signed in) → the code screen; the one code asks for the key and the repositories | Tab, Enter | tap | T M D |
| 41.03 | Copy the sign-in code | click "Copy code" (the code box selects all) | — | tap Copy code | T M D |
| 41.04 | Open GitHub's device page | click "Open github.com/login/device" (a new tab) and type the code there; k7 waits, asking while the page is visible | — | tap | T M D |
| 41.05 | Try the sign-in again | click "Try again" after "The code expired.", "Sign-in was cancelled on GitHub." or a network error | — | tap | T M D |
| 41.06 | Cancel the sign-in | click Cancel under the code (back to Services' GitHub row) | — | tap | T M D |
| 41.07 | Allow repositories | Services → GitHub, a sign-in GitHub granted without the repository permission: click "Allow repositories" → one more code (copy, open and try again as 41.03–41.05); the git pages' GitHub tab shows no picker until it is done | — | same | T M D |
| 41.08 | Sign in to a Forgejo server | Services → the server's section: click "Sign in" (Not signed in) or "Sign in again" ("The sign-in ended: sign in again") → "Asking <server> to sign in…", then the sign-in link | Tab, Enter | tap | T M D |
| 41.09 | Open the server's sign-in page | click "Open <server> to sign in" (a new tab) and allow k7 there; "Waiting for you to allow k7 on <server>…" until the server answers | — | tap | T M D |
| 41.10† | Try the Forgejo sign-in again | click "Try again" after "The sign-in expired.", "Sign-in was refused on <server>." or "Sign-in failed: …" | — | tap | T M D |
| 41.11 | Cancel the Forgejo sign-in | click Cancel under the link or the error (back to the server's section) | — | tap | T M D |

## 42 Forge repositories

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 42.01 | Filter the repositories | type in "Filter repositories" (matches owner/name; one per account's picker) | — | tap, type | T M D |
| 42.02 | Pick a repository | click a row of an account's picker (one per signed-in account, under "Signed in as @login" for GitHub or "Signed in as @login on <server>" for a Forgejo server; k7 workspaces first, then Your repositories, each most recently pushed first; a Private badge and the pushed-ago): Sync settings connects at once; Clone fills the address and the name, moves to Other host and waits for Clone | Tab, Enter | tap | T M D |
| 42.03 | Start a new repository (Sync settings only) | click "New repository" in an account's picker → the name prefilled with the workspace's, Private on | Tab, Enter | tap | T M D |
| 42.04 | Name the new repository | type in "Repository name" | — | tap, type | T M D |
| 42.05 | Make the new repository public | turn "Private repository" off | Space | tap | T M D |
| 42.06 | Create and connect | click "Create and connect" (the repository gets the `k7-workspace` topic); a name already taken reads "A repository named <name> already exists on <server>." | — | tap | T M D |
| 42.07 | Cancel the new repository | click Cancel (back to the list) | — | tap | T M D |

## 43 Clone a repository

### Git screens

| ID | Interaction | Mouse / trackpad | Keyboard | Touch | Devices |
|---|---|---|---|---|---|
| 29.02† | Name the clone | Clone's Other host tab: type in "Name of the workspace" (it follows the address, or the picked repository's name, until typed; a taken or bad name is said under it) | — | tap, type | T M D |
| 43.01 | Add or sign in to a Forgejo server in the Forgejo tab | Forgejo tab with no signed-in server: the Forgejo servers' section of Settings → Services, in place: "Add a Forgejo server" (23.17–23.22), "Sign in" (41.08) | Tab, Enter | tap | T M D |

## Gaps found while listing

- The ruler's drags (a range across every lane, a vertical drag to zoom) and the keyboard grab have no touch way; clips, edges, fades, lane headers, shots and automation points take a hold, then a drag.
- The piano roll lane takes a one-finger swipe as a native scroll, which competes with drawing notes and framing rectangles by touch.
- `apps/k7-daw-frontend/docs/timeline.md` says the mouse wheel zooms the timeline; the code zooms only on ⌘/Ctrl+wheel or a pinch.
- The bare wheel zooms in the audio clip panel and the graph but scrolls in the timeline, the piano roll and the waveform strip.
- The clip tooltip says "shift: off the grid", but ⇧ pressed on a clip starts a rectangle instead; it frees the grid only once a drag has begun.
- HTML5 drag and drop rows have no touch path: OS files and resources onto lanes, timeline tabs, file tree moves, parameter binding in the module builder, records onto decks.
- Double-tap rows rely on the browser's `dblclick` and are untested on iOS and Android (gain reset, stretch points, curve points, graph zoom, turntables resets).
- Takes mode has no key in either panel, and the timeline's mute tool has none either.
- Hover-only affordances have no touch twin: marker names, shot spans, lint messages, minimap peek, command bar row reasons, the cable `+`.
- The audio panel's loop braces drag free while the piano roll's snap (Alt frees); the two describe the same widget differently.
- A click on the wave (standard and warp views) and on the waveform strip does nothing: no seek is wired.
- "fix", beside the clip light, has no touch path.
- Binding a shortcut on the Commands page needs a hardware keyboard.
- A chain or OUT card opens no menu on right-click, unlike every other card.
- Removing a takes crossfade needs Alt, and sizing a timeline crossfade needs a drag: touch gets only the default.
- The App settings boards of the General, Devices, Downloads and About tabs still draw four tabs; only the Services boards (screens 23 and 41) draw the Services tab.
- Clone a repository keeps a name field (29.02), which the CloneHost boards do not draw.
