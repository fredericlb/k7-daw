# Settings

k7 has three places for settings, depending on what they belong to:

| Where | Opens with | Holds |
|---|---|---|
| **Settings panel** in the editor | the cog at the right of the footer, or `ui settings` | Editor, I/O, project and account |
| **Commands page** | the ⌘ button at the top of the settings panel, or `ui settings commands` | Your names, icons, keys and menus for commands |
| **Settings page** on the home page | `/settings`, from the user menu | What a browser may change on this device: language, separation chains, AI key, identity, paired devices |
| **Engine page** on the home page | `/settings/engine`, from the user menu | The engine of this device: on the tray, this computer's engine; in the phone app, the phone's |

Where each setting is **kept** matters:

| Setting | Kept |
|---|---|
| Command names, icons, keys, menus and order | On this device: the same in every browser open on it |
| Interface language | In this browser only |
| Vim mode | On the machine that holds your files |
| AI key and model | On this device; the key is never shown again |
| Separation chains, channel labels, headphone cue | On the engine's machine |
| The project's console | In the project's files |

## The settings panel

The panel docks at the right of the window, beside what it configures. `Esc` or its ✕ closes it; it reopens on the tab you left. Each tab is a command, so you can also go straight to one.

### Editor (`ui settings editor`)

**Vim mode**: vim key bindings in the text editor, on or off. Same as `ui vim`.

### I/O (`ui settings io`)

Every input and output module of the song — the project's own and those inherited from the workspace — each with a live level meter. The meters only run while this tab is open. Channels with no device say so. The audio devices themselves are covered in the Engines section.

### Project (`ui settings project`)

**Console**: a summing console over the whole mix, chosen from those your libraries hold, or *None*. Its channel comes after every track's strip and its buss first on the master. Same as `project console <system>`.

### Account (`ui settings account`)

The device's identity, and:

- **Engine** (`ui engine`) — back to the engine picker,
- **Quit workspace** (`ui workspace quit`),
- **Language** — English or Français.

## The Commands page

`ui settings commands` opens a full page listing every command, filterable by the thing it acts on (track, event, node…) and by text. For each command you can:

- **Rename it**, separately in each language. Your name replaces the sentence the command bar lists and the label of right-click menus.
- **Give it an icon**, shown in menus.
- **Bind a key**: click the key field, then press a key. `Esc` cancels, `Backspace` removes the key (including a default one), `Shift` combines with the key, and `Ctrl`, `⌘` and `Alt` chords stay the browser's. A key given to a command is taken from the command that had it.
- **Choose the right-click menus** that offer it (tracks, events, nodes, the ruler…), or none.
- **Move it up or down** to set the order menus list commands in.
- **Give it back to the app**: drop your changes for that command.

The page edits a draft: **Save** writes it to your account; leaving with unsaved changes asks first. Only plain one-key shortcuts can be rebound; the context-dependent keys listed in [Keyboard shortcuts](./12-keys.md) stay as they are.

## The home page's Settings page

Reached from the user menu at the foot of the home page's sidebar. It holds what a browser can change on this device, under two tabs: General and Devices. The engine, the audio interface and the machine's own settings are set on that machine, on the tray's engine page or in the command-line engine's config file; the phone's engine is on the Engine page.

### Language

English or Français. Kept in this browser only: another machine starts in English. k7 does not follow the language your browser asks for. `ui language fr` does the same from the editor.

### Separation chains

Which of your libraries' offline chains this machine uses to **separate a record into stems**, and which one makes an **instrumental**. One answer for the machine, whatever project is open. A chain whose model is not on the machine offers a **Download the model** button. Same as `system stems` and `system instrumental`.

### AI

The Claude API key and model the module builder's **AI organize** uses. Paste a key and press Save: it is stored on this device and never reaches another; only a short hint (`sk-ant-…wxyz`) is ever shown again. **Remove** deletes it. Without a key, the module builder shows no AI organize button. The model field takes a Claude model id; left empty, the default model is used.

### Paired devices

Phones, tablets and other computers on your local network allowed to drive this machine's engine, each with when it was paired and last seen, and a **Revoke** button. Requests waiting for an answer appear here too. This list can only be managed from the machine running the engine. See [Driving k7 from another device](./18-second-device.md).

### Identity

The name and email that sign every git commit made from this device, and head the user menu. The same fields are asked once by the commit dialog when empty.

## The Engine page

**Engine** in the user menu opens `/settings/engine`: the engines this device may use, each measured from this browser. On a computer running the tray it opens on this computer's engine, with that list as its last tab (see [k7 on your computer](./82-desktop-app.md#the-engine-page)); in the phone app it opens on the phone's engine.
