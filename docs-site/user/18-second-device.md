# Driving k7 from another device

A tablet beside your decks, a phone in your hand on stage: a second device can drive the **same engine, in the same session** as the computer running your project. What you do on one is done on the other.

A second device opens **one surface alone**: the DJ desk, the timeline, the master strip or the picture frame, full screen, with the transport at the bottom.

## Surfaces

A project declares its special views — its *surfaces* — in its `surfaces/` folder (see the Song file reference). Each one has a name (its id) and a type: `timeline`, `turntables`, `mastering` or `viewport`. Any of them can be opened alone on another device.

## Opening a surface from another device

Both devices run k7 and are on the same network. On the computer that has the project open, turn on **Available on the local network** in the engine settings. On the second device, open the **Surfaces** tab of the home page.

The tab lists, for every project open on your network, the surfaces this device can open: the device's name over its project's name, then a row per surface with its type. Tap one to open it. The list refreshes every five seconds.

A row that cannot be opened says why:

- **Not answering**: the device is listed but its engine did not answer.
- **Not trusted**: turn on **Available on the local network** on that device.
- **Update k7 on** a device: the two k7 versions do not speak the same protocol.
- **Drawn in k7's window only**: that surface type has no drawing for a second device.

**Also on** a device means another device already shows that surface; two devices may show the same one. If nothing is found, the page says to turn on **Available on the local network** on the device that has the project open.

The list only offers the project already open on that device: joining it never moves its engine to another project. A device that has no project open, or whose project declares no surface, says so.

## Only one device per pane

The computer that has the project open cannot open the pane another device shows. In its header the pane is dimmed and says "In use on" the device's name when you press it. If you open on the second device the pane the computer is on, the second device takes it and the computer moves to the next free pane, with a message. When the second device leaves, the pane is free again within a couple of seconds.

## A surface on its own

A surface opened on another device shows:

- the surface itself, full screen, with no way to change surface;
- at the bottom, the **transport** (play, stop and the clock);
- one **command line** and a full-screen button;
- a round **Home** button at the top left, which goes back to the Surfaces tab.

The screen stays awake while a surface is open.

The keyboard works as in the editor, with your own key bindings: `Space` plays, `Shift+Space` replays, the timeline keys move the caret, zoom and loop, and double-`Shift` focuses the command line. Commands that only make sense in the full editor (opening panels, other pages…) answer with a message instead of acting. Right-click entries that need an argument are pre-written in the command line.

`ui external <surface>` copies a surface's link and shows it on the status line; that link opens the surface alone in a browser on the same computer.

## Pairing a device on your local network

When a device reaches an engine directly over your local network, the engine only answers devices its machine has approved.

A card says **This device isn't paired** with the machine's name:

1. Tap **Pair this device**.
2. The device shows a **code**.
3. On the machine running the engine, a card appears at the top right: "*<device>* wants to pair", with its network address and the same code. Check the code matches, then click **Approve** (or **Deny**).
4. The device is paired; the approval is remembered.

A request nobody answers expires after two minutes. The machine's **Paired devices** list on the Settings page shows every paired device with **Revoke**. It can only be managed from the machine running the engine. More on engines and pairing machines is in the Engines section.
