# The window

K7 DAW is k7 in a window of its own, on Windows, Linux and macOS. The window shows the page k7's backend
serves on `127.0.0.1`; the backend is the one `docs/app.md` describes.

## One window, the page's own

| choice | why |
|---|---|
| one window, label `main`, made at startup on the bundled splash (`ui/index.html`), navigated to the backend's page once it answers | the page's origin must be `http://127.0.0.1:<port>` (storage, secure context); a remote URL loaded before the backend listens shows an error page, and the splash can say what the backend is doing |
| the host writes its status line into the splash with `eval`, and reloads the page after a gap in the backend | no IPC is needed for either |
| no capabilities file, no IPC for the page; the host marks the window with `window.k7Desktop = { release }` from an init script | the page is the same bundle every host serves (`apps/k7/frontend/docs/localApp.md`); a loopback page with native powers would hand any injected script the machine. The phones mark their views the same way |
| closing the window hides it on a Mac, where Quit (Cmd-Q, the app menu) is the only way out, and quits the app elsewhere | a Mac app outlives its windows and the Dock icon brings it back; Windows and Linux have no such entry, so a hidden window would be unreachable |
| the window comes up on every launch, a second launch, and a Dock click on a Mac | one app, one window |
| the window's size, position and maximized state are remembered (tauri-plugin-window-state), its visibility is not; 1440x900 by default, 960x600 at least | visibility belongs to the launch above |
| HTML5 drag and drop reaches the page (`disable_drag_drop_handler`) | the file tree and the timeline take file drops; Tauri's native handler would swallow them |
| devtools only in debug builds | a release user gets no inspector |

A scratch run (`K7_DESKTOP_SCRATCH`) keeps the window state and the web storage out of the person's
folders: it is a second app on purpose, for a test of its own.

## What the webview may do

`src/handlers.rs` answers what the webview asks the host.

| choice | why |
|---|---|
| a link stays in the window only on k7's own origin, the splash, `blob:`, `data:` and `about:`; any other `http(s)` or `mailto` link opens in the system browser, and every other scheme goes nowhere | k7.rocks, the docs and git hosts belong in the browser, and the window has no address bar to come back from |
| a new window (`target="_blank"`: About, Notices, the docs site) is always denied, its URL opened in the system browser | WebView2 would open a raw popup, and WKWebView and WebKitGTK drop the click |
| a download that is not an export goes to the Downloads folder, never over a file ("name (2).ext"), then a dialog says where, with Show in Folder | `<a download>` on blobs and same-origin files is how the rest of k7 saves; every webview routes it to the host, which has to say where it went. An export does not use it: the person chooses its folder (`apps/k7/frontend/docs/export.md`) |
| the folder picker is Tauri's dialog plugin, called by the front through a trait the app installs (`src/exports.rs`), not by the page | the page has no IPC (above); the front is in this process and already holds the page's session, so a route answers where a command would |
| the camera, the microphone and MIDI are allowed for k7's origin; the screen, the position and notifications are denied; the rest follows the platform | k7 uses the camera and Web MIDI, and records audio through the engine; macOS still asks once (`macos.md`) |

## The menu

One menu per OS (`src/appmenu.rs`): the Mac's menu bar holds K7 DAW (About, Engine Settings, Open in Browser,
Restart, Hide, Quit), Edit, View (Reload, Full Screen), Window and Help (the docs, Show Logs); Windows and Linux
carry the same entries in the window's menu, under File (Engine Settings, Open in Browser, Restart, Quit), Edit, View
and Help.

| choice | why |
|---|---|
| Open in Browser, Engine Settings, Restart and Show Logs are menu entries and nothing else | the page has no native power (`docs/app.md`), so the menu is where the host's own actions live |
| Open in Browser opens the same address in the browser | the way to Web MIDI where the window has none (`webviews.md`) |
| Restart stops the front as Quit does and starts it again | every state the app can end in has a way out short of quitting it |
| the Edit menu holds the predefined Undo, Redo, Cut, Copy, Paste and Select All | WKWebView gets Cmd-C, V, X, A and Z only through those key equivalents; a key the page handles itself and cancels (k7's Cmd-Z) still reaches the page first |
| Close (Cmd-W) hides the window, on a Mac only | the same rule as the close button |

A webview calls the navigation handler for subframes too: a cross-origin iframe would be sent to the browser, so a page
that embeds one needs the handler taught first (today's only frame is `srcdoc`, `about:`). The page keeps Tauri's own
injected internals, but there is no capability and no command handler, so every call from it is refused.

After a gap in the backend the window reloads the page: what the backend never received is lost with it either way.

## Names

The product is **K7 DAW**; the binary and the Cargo package are `k7-desktop` and the bundle identifier is
`org.parapluie.studio.engine-tray` (`docs/app.md`). Release files are `k7-daw_<version>_<os>-<arch>.<ext>`.
