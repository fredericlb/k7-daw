# The window

K7 DAW is k7 in a window of its own, on Windows, Linux and macOS. The window shows the page k7's backend
serves on `127.0.0.1`; the tray icon and the backend are the ones `docs/tray.md` describes.

## One window, the page's own

| choice | why |
|---|---|
| one window, label `main`, made at startup on the bundled splash (`ui/index.html`), navigated to the backend's page once it answers | the page's origin must be `http://127.0.0.1:<port>` (storage, secure context); a remote URL loaded before the backend listens shows an error page, and the splash can say what the backend is doing |
| the host writes its status line into the splash with `eval`, and reloads the page after a gap in the backend | no IPC is needed for either |
| no capabilities file, no IPC for the page; the host marks the window with `window.k7Desktop = { release }` from an init script | the page is the same bundle every host serves (`apps/k7/frontend/docs/localApp.md`); a loopback page with native powers would hand any injected script the machine. The phones mark their views the same way |
| closing the window hides it; Quit (tray, Cmd-Q, the app menu) is the only way out | the engine also serves other devices and k7.rocks through the tunnel, so quitting is deliberate |
| the window comes up on a normal launch, "Open K7 DAW", a second launch, and a Dock click on a Mac; a login item starts it hidden (`--autostart`) | a login should not pop a window |
| the window's size, position and maximized state are remembered (tauri-plugin-window-state), its visibility is not; 1440x900 by default, 960x600 at least | visibility belongs to the launch above |
| HTML5 drag and drop reaches the page (`disable_drag_drop_handler`) | the file tree and the timeline take file drops; Tauri's native handler would swallow them |
| devtools only in debug builds | a release user gets no inspector |

A scratch run (`K7_TRAY_SCRATCH`) keeps the window state, the login item and the web storage out of the person's
folders: it is a second app on purpose, for a test of its own.

## What the webview may do

`src/handlers.rs` answers what the webview asks the host.

| choice | why |
|---|---|
| a link stays in the window only on k7's own origin, the splash, `blob:`, `data:` and `about:`; any other `http(s)` or `mailto` link opens in the system browser, and every other scheme goes nowhere | k7.rocks, the docs and git hosts belong in the browser, and the window has no address bar to come back from |
| a new window (`target="_blank"`: About, Notices, the docs site) is always denied, its URL opened in the system browser | WebView2 would open a raw popup, and WKWebView and WebKitGTK drop the click |
| a download goes to the Downloads folder, never over a file ("name (2).ext"), then a dialog says where, with Show in Folder | `<a download>` on blobs and same-origin files is how k7 exports; every webview routes it to the host, which has to say where it went |
| the camera, the microphone and MIDI are allowed for k7's origin; the screen, the position and notifications are denied; the rest follows the platform | k7 uses the camera and Web MIDI, and records audio through the engine; macOS still asks once (`macos.md`) |

## The menu bar

macOS has one (`src/appmenu.rs`): K7 DAW (About, Engine Settings, Updates, Hide, Quit), Edit, View (Reload,
Full Screen), Window, Help. Windows and Linux have none: the tray menu and the page carry everything.

| choice | why |
|---|---|
| the Edit menu holds the predefined Undo, Redo, Cut, Copy, Paste and Select All | WKWebView gets Cmd-C, V, X, A and Z only through those key equivalents; a key the page handles itself and cancels (k7's Cmd-Z) still reaches the page first |
| Close (Cmd-W) hides the window | the same rule as the close button |
| the Dock icon stays, with the status item | a windowed app without a Dock entry, an app menu and Cmd-Tab is not a Mac app |

A webview calls the navigation handler for subframes too: a cross-origin iframe would be sent to the browser, so a page
that embeds one needs the handler taught first (today's only frame is `srcdoc`, `about:`). The page keeps Tauri's own
injected internals, but there is no capability and no command handler, so every call from it is refused.

After a gap in the backend the window reloads the page: what the backend never received is lost with it either way.

## The tray menu

"Open K7 DAW" brings the window up; "Open in Browser" opens the same address in the browser, which is also the way
to Web MIDI where the window has none (`webviews.md`); "Engine Settings…" opens `/settings/engine` in the window.

## Launch at login

The login item is called `K7 DAW` and carries `--autostart`. An entry written before the window existed carries no
argument, so it is registered again once at the first start; the entry under the tray's old name `k7-tray` is
removed and the new one enabled in its place.

## Names

The product is **K7 DAW**; the binary and the Cargo package stay `k7-tray` and the bundle identifier stays
`org.parapluie.studio.engine-tray` (`docs/tray.md`). Release files are `k7-daw_<stamp>_<os>-<arch>.<ext>`; the
update feed stays `app_downloads/tray/<channel>/latest.json`, which installed apps poll.
