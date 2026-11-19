# K7 DAW: k7 on your computer

**K7 DAW** puts all of k7 on your own computer: the editor, k7's own backend, the supervisor and the engine. k7 opens in a window of its own, and a small icon in the system tray (the menu bar on a Mac) shows what the engine is doing. It runs on Windows, Linux and Macs with Apple silicon (macOS 13 or later).

Install it when you want to make music on your own machine without depending on a network connection.

For what the engine and supervisor are, see [Engines](./80-engines.md).

## Install

Download the file for your computer from the [release page](https://github.com/fredericlb/k7-daw/releases) of k7 on GitHub:

| Platform | File |
|---|---|
| Windows | `k7-daw_<version>_windows-<arch>.exe` (an installer) |
| macOS | `k7-daw_<version>_macos-arm64.dmg` |
| Linux | `k7-daw_<version>_linux-<arch>.AppImage`, or `.deb` for Debian and Ubuntu |

On Windows, SmartScreen may warn on first launch: confirm that you want to open it. The Windows installer replaces an older **k7-tray** install, which it removes. On a Mac, open the `.dmg` and drag K7 DAW to Applications.

The first time you record, your system asks for permission to use the microphone, and the first time you use a camera lane, the camera. On Linux, the `.deb` pulls in GStreamer, which the window needs for audio and video.

Only one copy of K7 DAW runs at a time. Launching it again brings its window forward. Closing the window does not quit: the engine keeps running in the tray, and **Open K7 DAW** (or the Dock icon on a Mac) brings the window back.

## The tray menu

Click the icon to open its menu:

| Entry | What it does |
|---|---|
| The status line | What k7 is doing: *Starting K7 DAW…*, *Starting the engine…*, *● Engine running · :8765*, *○ Engine stopped*, or a line starting with ⚠ that says what went wrong. While an instrument downloads, a second line shows its progress. |
| **Open K7 DAW** | Brings the window forward. |
| **Open in Browser** | Opens k7 in your web browser instead (see [Web MIDI](#midi-keyboards)). |
| **Engine Settings…** | Opens the engine's settings page in the window (see [The engine page](#the-engine-page)). |
| **Restart K7 DAW** | Stops k7 and the engine and starts them again. |
| **Launch at Login** | Starts K7 DAW, hidden in the tray, when you log in to your computer. |
| **Show Logs** | Opens the log of k7 and its engine. |
| **Quit K7 DAW** | Stops the engine and k7, then quits. |

The icon itself shows the same state at a glance: running, stopped, starting (it turns), or a problem.

On a Mac, the menu bar of K7 DAW also holds Engine Settings…, Reload, Show Logs and the usual Edit, Window and Help menus.

## The window

The window is k7, the same page you would get in a browser at `http://127.0.0.1:18789`; that address is your own computer, and nothing you do there goes through the internet. You land on k7's home page as the computer's **local user**: you can create workspaces and projects at once, with no account. Your first workspace starts empty.

The library of modules and chains, `@k7-core`, is copied onto your computer the first time it is needed, from the copy bundled with K7 DAW. From then on it is yours to edit like any library.

Links to other sites (the docs, k7.rocks, a git host) open in your browser. Exporting a file saves it in your **Downloads** folder and tells you where, with a **Show in Folder** button.

### MIDI keyboards

Recording from a MIDI keyboard needs Web MIDI, which the window has on Windows and not on macOS or Linux. There, choose **Open in Browser** (Chrome or Edge) to record MIDI; the same projects and engine are behind it.

### Working offline

Everything k7 needs is on your computer: the page, its backend, your files and the engine. You can edit and play your projects with no network at all. Only git, when you push or pull a workspace, needs a connection.

## Moving workspaces to another computer

There is no account: connect a workspace to a git repository and pull it on the other computer. See [Sync, history and conflicts](./19-sync-and-history.md).

## The engine page

**Engine Settings…** in the tray menu, or **Engine** in the user menu at the bottom of the home page's sidebar, opens `/settings/engine`, the page that controls this computer's engine. Its last tab, **Engines**, lists the engines this device may use. Opened anywhere other than K7 DAW's address, the page lists only those engines.

### This machine's engine

The status line reads *Running, on port 8765*, *Stopped*, *Stopped — start it again when you need it*, or *Stopped by an error: …*. Under it is the engine's id.

| Button | What it does |
|---|---|
| **Start** | Starts the engine. |
| **Stop** | Stops the engine. It stays stopped until you start it. |
| **Restart** | Stops and starts the engine. Use it after a plugin misbehaves. |

If the engine crashes, the supervisor restarts it by itself, waiting a little longer after each failed attempt (up to 30 seconds). The editor then shows *The engine crashed and was restarted. The project is being sent to it again; anything playing stopped.*

### Audio interfaces

Every interface the engine sees, in your order of preference. The engine uses the **first one that is plugged in**. Use the up and down arrows to move an interface. Each row says its backend (for example *CoreAudio*, *Windows Audio*, *ASIO*), and marks the one **in use**, the one taking the **inputs** when that is a different device, and the ones **not plugged in**. Your order is saved even when an interface is unplugged, so plugging it back in moves the engine to it. On Windows, when the engine is on the shared Windows output while your interface has its own ASIO driver, the page offers **Use ASIO**. See [Audio interfaces, MIDI and plugins](./84-audio-midi-plugins.md).

### Downloads

The sampled instruments the engine can play (see the list in [Engines](./80-engines.md#sampled-instruments)). Nothing is downloaded until you ask: the piano alone is 750 MB.

| Row says | Button |
|---|---|
| the size only | **Download** |
| *waiting for the download before it* | **Cancel** |
| a progress bar, or *unpacking* | none: a download cannot be stopped halfway |
| *download failed: …* | **Retry** |
| *installed* | **Remove** |

### Plugins

How many plugins the engine found on this computer, and the list with each plugin's maker and format. *Scanning…* shows while a scan runs. Click **Rescan** after installing a plugin.

## Updates

K7 DAW does not update itself. When a newer version is out, download its file from the release page and install it the way you installed the first one: it replaces the old one, and your workspaces stay where they are.

## Ports, and running beside other k7 pieces

| Port | Used by |
|---|---|
| 18789 | k7 in its window and in your browser, on this computer only |
| 8765 | The engine, which other devices on your network can reach once you approve them |

K7 DAW's engine is the computer's engine: the same one, in the same `~/.k7` folder and on the same port, as the [command-line engine](./81-command-line-engine.md). Run one or the other on a computer, not both. A k7 development session uses port 8789 and does not get in the way.

A phone or tablet on the same network can drive the engine once you approve it. See [Reaching an engine](./85-connecting-machines.md).

## Where K7 DAW keeps your files

Your workspaces and preferences go into the operating system's folder for K7 DAW's data (`org.parapluie.studio.engine-tray`, for example `%APPDATA%` on Windows):

| Inside that folder | What it holds |
|---|---|
| `workspaces/` | Your workspaces and projects |
| `mediatek/` | The data folder of K7 DAW's backend |
| `prefs/` | Your preferences |

The engine's state is in `~/.k7`: its identity, settings (`engine-host.json`), plugin scan, paired devices, downloaded instruments, and the log, `.studio.log`, which **Show Logs** opens.

## Quitting

**Quit K7 DAW** stops the engine first, then k7, then the app. A k7 page open in your browser stops answering until you start K7 DAW again.
