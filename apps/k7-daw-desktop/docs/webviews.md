# The webview on each system

K7 DAW's window is the system's own webview: WebView2 on Windows, WKWebView on macOS, WebKitGTK on Linux. k7's page
uses what is below, so what a webview lacks is a limit of K7 DAW and not of k7.

| feature | WebView2 | WKWebView (macOS 13+) | WebKitGTK 4.1 |
|---|---|---|---|
| secure context on `http://127.0.0.1` | yes | yes | yes |
| WebAssembly, WebGL2, AudioWorklet | yes | yes | yes (WebGL needs a GPU) |
| camera (getUserMedia) | yes | yes, after the usage string and the entitlement | after the host turns media stream on |
| Web MIDI | yes | no | no |
| WebRTC | yes | yes | where the distribution built it; the local engine does not need it |
| MediaRecorder | yes (webm) | yes (mp4) | through GStreamer |
| `<a download>` | host's download handler | host's download handler | host's download handler |
| `confirm()`, `alert()` | native | not implemented: answers false | GTK dialogs |
| element fullscreen | the webview's bounds only | yes | the whole window |
| file drops | page's, with the host's handler off | same | same |

| choice | why |
|---|---|
| Web MIDI is reached through the menu's "Open in Browser" on a Mac and on Linux | the webview has none; a native MIDI bridge is its own design |
| the app asks its own question before leaving a project with unsaved work (`askConfirm`), never `window.confirm` | WKWebView's `confirm()` returns false without showing anything |
| the window runs no service worker, and drops one it holds | an installed precache would serve the previous release's page after an update |
| Linux: media stream, WebRTC and media source are switched on (`src/webview_linux.rs`) | WebKitGTK ships them off, so `getUserMedia` does not exist |
| Linux: the DMA-BUF renderer is turned off when NVIDIA's proprietary driver is loaded and the variable is not set | WebKitGTK 2.42 and later draw a blank window with it |
| Linux packages depend on GStreamer's base and good plugins and recommend bad and libav; the AppImage bundles the media framework | WebKitGTK decodes, plays and records through GStreamer |
| Linux is built on Ubuntu 22.04 (webkit2gtk-4.1); Debian 12 and Ubuntu 22.04 are the floor | the oldest system whose WebKitGTK k7 runs on |
| Windows: the installer embeds the WebView2 bootstrapper | it installs the runtime where it is missing (Windows 10 and 11 ship it); the bootstrapper downloads it, so a machine without the runtime and without a network needs an `offlineInstaller` build |
| no F11 or native full screen on Windows and Linux; the page's own fullscreen request is best effort (the webview's bounds only, on Windows) | a window key handler is future work |
