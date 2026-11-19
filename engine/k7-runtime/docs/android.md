# The engine on Android

The same engine and the same supervisor, built for arm64 Android by their `android-arm64` presets
and run by the app in `engine/k7-android`, which drives the supervisor as k7's backend does under the tray. The supervisor lists the
phone at the studios it is connected to, exactly as on a computer, and the engine serves the same
WebSocket protocol on port 8765, bound to every interface.

**It lives as long as `MainActivity`**, and leaving it deregisters the engine, stops it and ends the
process. That teardown runs on a thread of its own, never the UI thread, since its stops wait for
seconds and a blocked main thread is an ANR; the backend gets longer than its own 8 s stop, so its
file services are not killed mid-teardown. Its two services, `MediaPlaybackService` and `EngineService`, keep that process in
front while a page plays or the engine is at work (below), and start nothing of their own.

The iOS app (`docs/ios.md`) is this architecture again. It shares this app's shell page (`engine/k7-android/web`)
and its three page scripts — account, top colour, media session — which accept either app's channel.

## Standalone: the phone runs its own studio

Like the tray on a computer (`apps/k7-tray/docs/tray.md`), the phone needs no server: it runs
**k7's front itself**, and the engine. A studio
elsewhere is something it can ADD, never something it needs. `BackendHost.kt` is the tray's
`backend.rs` on a phone.

| choice | why |
|---|---|
| the tray's port, environment and folders, read from `apps/k7/backend-rs/standalone.json` | one table for both, whose values `apps/k7/backend-rs/docs/standalone.md` explains; the port is a `BuildConfig` constant, since the views compare origins before anything is unpacked |
| the local user when signed out | a view opens on `/auth/local`, and falls back to it whenever `/auth/me` says nobody — what a page on the tray's backend does (`apps/k7/frontend/docs/localApp.md`), with the signal the views already had |
| k7's view opens straight on `/` when the WebView holds a `studio.sid` cookie (`BackendHost.home`) | `/auth/local` is a redirect before any app code, on every cold start; a cookie whose session is gone costs one bounce, since the page then says nobody and the fallback above loads `/auth/local` |
| the phone's own studio pinned FIRST in the supervisor's list, before it starts | the supervisor's saved list overrides `$STUDIO_REGISTRY_URL`, so a phone that already listed a studio would never see its own otherwise |
| the front runs in the app's process, with no node and no child for it | the front holds the file hub and every account's file service itself (`apps/k7/backend-rs/docs/files.md`); the page, the library it seeds and its code root ride in the zip (`apps/k7/backend-rs/scripts/stageBundle.mjs`) |
| one `backend.zip` asset, unpacked once per install | the front reads files, not assets; the APK's `lastUpdateTime` marks the copy |
| the Rust front's code root rides in `backend.zip` as `code/` (`apps/k7/backend-rs/scripts/stageCodeRoot.mjs`, some 700 KB), named to the front as `K7_CODE_ROOT` | the front's actor reads `@meta`, `commands.yaml`, the demos and the sample video there (`apps/k7/backend-rs/docs/service.md`); a folder of its own, beside the page and the library |
| an update writes only the files whose CRC or size changed, and deletes those the new bundle lacks | the bundle is some 20 MB unpacked, and an update rewriting all of it holds the start back; `.entries` keeps each file's CRC and size, the ones Gradle's `Zip` writes in every local header. An entry without them is always written, and the `.installed` marker is removed first and written last, so an unpack cut short is redone |
| cleartext allowed for `127.0.0.1` only (`engine/k7-android/app/src/main/res/xml/network_security_config.xml`) | the local studio is plain http, as under the tray, and every other host stays https |
| the views wait for the backend before loading it | a first run spends a few seconds unpacking, and a load that fails then would put the error screen up for nothing |
| the front answers `/api/health` at once, and holds the paths that create workspaces until the bundled library is installed | the start screen hands over on that answer, and a workspace created before the library exists has none; the front owns the wait (`apps/k7/backend-rs/docs/standalone.md`, "Ready means the library is there"), so `BackendHost` and the iPhone's need no probe of their own |
| the shell page shows the start — "Starting k7…", with the unpack's percentage — and a failed one says why, with a Retry (`web/src/Backend.tsx`) | the phone's own studio is the one a first launch opens, so a start that never ends must not be a blank screen; the percentage is the bytes of `backend.zip` read, which is stored uncompressed in the APK, so its length is known before the unpack |
| the zip streamed straight out of the APK's assets, never copied to disk first | the extracted tree is the only copy the phone keeps, so there is nothing to delete after it |
| native libraries extracted at install (`useLegacyPackaging`), and no R8 | the supervisor is EXECUTED, which needs real files; R8 would need keep rules for Capacitor's reflective bridge and the JNI entry points that nobody has written, and a release that breaks at run time costs more than the megabytes |

**The studio's port is the Rust front's** (`RustFront.kt`, `apps/k7/backend-rs/docs/embed.md`): it runs in
the app's process on `STANDALONE_PORT`, the one every view loads.

| choice | why |
|---|---|
| the front started once per process | a Retry after a failed start redoes only what failed |
| the readiness wait asks the studio's port for `/api/health` | the front is the only server there is |
| the engine core in the front (`K7_ENGINE_CORE=front`, `apps/k7/backend-rs/docs/engine.md`), the machine's engine not (`K7_LOCAL_ENGINE` unset) | the registry, the relay and the tunnel hub are the same Rust on every host; the supervisor is `SupervisorHost.kt`'s, which no front runs here, so there is no local engine for the front to take |
| a front that cannot start fails the start, with its reason | without it no view loads anything, and a start screen waiting forever says nothing |
| the compiled modules in `front-cache` beside the front's state (`K7_FRONT_CACHE`) | the same Rust as the tray's; the app's process runs from no folder the front could search up from, and a compiled module is machine code, so its folder is the app's own; a cold one costs the first scan some 20 s |
| `BackendHost.kt` sets no file-service variable: the front runs all fifteen native pieces answering, within the embedded 192 MB heap; `engine`, `media` and `runtime` degrade as the front does with no engine bound, and none is proved on a device yet (5-13) | the pieces are the only file service there is |
| the file hub in the front (`K7_FILES=front`, `apps/k7/backend-rs/docs/files.md`): every account's file service and the libraries' are the front's own | one hub, the same Rust as the tray's |
| the front given the table's environment, its folders, the session secret, the page's folder (`K7_FRONTEND_DIST`) and the switches | it answers every route and reads the settings those routes read |

**A start that fails is given up on, and the user's retry redoes it.** A front that cannot start — a full disk,
a missing library — fails the same way on every launch. Given up on, it tells the views waiting for it, and the
shell page shows "k7 could not start" with the reason and a Retry. There is one retry, `BackendHost.retry()`: the
shell page's Retry calls it, and so does a view asking for the front after it was given up on; it starts
again, reusing an unpack already done. Every step — the unpack and its percentage, the launch, ready, given up
on — is reported to the shell page as one status (`AppViews.backend()` and its `backend` event), which is what it draws.

| choice | why |
|---|---|
| a start that gets no answer within 60 s is given up on | a Retry must start again, not wait on the same wedged one |
| the output readers of the supervisor (`ProcessLines.kt`) end on any `IOException`, with one log line | a stop or a kill closes the child's pipe under a blocked read, and Android throws that as `InterruptedIOException`; uncaught on its thread, it kills the whole app |
| once a child has exited, its readers get 2 s to drain, then their stream is closed | a grandchild that inherited the pipe keeps it open past the exit, and the reader would then outlive its process |
| a kill that must kill — a stop past its timeout — is a SIGKILL found through `/proc` (`ChildKill.kt`) | Android's `destroyForcibly` sends SIGTERM, which a wedged or stopped child never acts on; its `Process` has no `pid()` |

**The engine may open the front's workspaces and data folder.** Its state is `filesDir/.k7`, and
the front's is `filesDir/standalone`, whose files it sends the engine by their own paths, not as
copies. The engine refuses any file outside its roots (`pathRoots.md`), so `EngineBridge.start` sets
`$STUDIO_ENGINE_ROOTS` to those two folders, named at build time from
`apps/k7/backend-rs/standalone.json` (`BackendHost.engineRoots`). Without them a project plays with
every clip, sample, patch and model dropped.

## The app is also where the studio is used: k7, full screen

`MainActivity` is a Capacitor `BridgeActivity`. With a studio connected, **the screen is k7, edge
to edge, and nothing of the app's own**: no header, no tab bar. k7 is the one app with a view, so
`APP_IDS` (`engine/k7-android/web/src/native.ts`) holds k7 alone. A phone has no room for a second
header above the app's own.

The status bar above the app is painted **the colour of that app's top edge**, so the app's
header reads as running up under it. That colour is measured, not declared: `assets/top-color.js`
composites every layer under the page's top pixel on a one-pixel canvas — AppHeader's tint is
translucent oklch over the page, which no computed style gives back as one colour — and reports
it again when the route or the theme changes. The status bar's icons follow its luminance.

**Full screen is a switch, off by default** (`Immersive.kt`): the status and navigation bars
hidden, a swipe from an edge showing them for a moment. It is set on k7's Engine page
(`setImmersive` on `k7Android`) and kept in the app's own preferences, not the account's: a
tablet left on k7 on a stand and a phone carried around want different answers, and one account
drives both. The activity applies it in `onCreate`, before any page loads, so the switch never
waits on a studio answering. The views need nothing of their own: their insets are the bars that
are SHOWN, so with the bars hidden the status-bar strip is zero high and only the cutout still
pads them.

**This phone's engine is set in k7's settings** (the home page's cog), because on the phone
k7 is the only screen there is: the engine's state, the private CA,
Quit (`apps/k7/frontend/docs/settings.md`). The page bundled in the APK, `engine/k7-android/web`,
is only seen while there is **no studio to open**, while the phone's own k7 is **still starting or
failed to**, or while the open one **fails to load** — then no app is on screen to reach settings in,
so it draws the same tray with a Retry. Its private CA form is
collapsed, since the phone's own k7 needs none.

The activity's theme is `AppTheme.Launch` (`res/values/themes.xml`), Capacitor's `AppTheme.NoActionBar`
with the startup splash as the window's background, so the phone shows the cassette from the first
frame (`apps/k7/frontend/docs/icons.md`). It stays a dark theme rather than a DayNight one. Every
studio page is dark-only (`@studio/ui/theme.css`, and k7's `color-scheme: dark`), so the
`prefers-color-scheme` a light theme would give the views changes nothing in them; what it would
change is the window behind a loading page and the native dialogs, a white flash and white boxes
over a dark app.

Each app's WebView is painted #07031F (`R.color.splash_background`) until its page draws: between the
shell's "launching" and k7's own splash it would otherwise show a black frame, which reads as a crash.

Two ways from the page to the native half:

- **The shell page** uses two plugins `MainActivity` registers itself, typed once in
  `web/src/native.ts`: **`K7Runtime`** (`EnginePlugin.kt`), `SupervisorHost`'s snapshots and
  commands; **`AppViews`** (`AppViewsPlugin.kt`), one native WebView per app over the page. Each is
  created the first time it is shown and only hidden after that, so an app keeps its state while
  another is on screen. With k7 the only app, that is what keeps it alive under the tray page.
- **The studio app** gets `window.k7Android` (`StudioBridge.kt`), an androidx.webkit
  WebMessageListener. Every app posts its top colour on it; **only k7, checked by origin on
  every message, may call the engine** (`apps/k7/frontend/src/settings/androidEngine.ts`). The
  object is offered to every origin because the studio can change under a view that already
  exists, and a listener's origin rules are fixed when it is added; the check is per message
  instead. k7's origin is whichever studio is open, possibly a remote one, so a script on it is
  not trusted with the supervisor: `command` takes the eight the engine page sends on a computer (`engine.start|stop|restart`,
  `audio.priority`, `lan.open`, `plugins.rescan`, `instruments.download|remove`, as the tray's front allows,
  `apps/k7/backend-rs/docs/localEngine.md`) and anything else is refused —
  `shutdown`, `session.drop`, `registry.deregister`, `weburi.*` and pairing included, which the panel never sends.
  `chooseStudio` takes `https` origins, or this phone's own loopback one; an origin no listed studio
  answers on asks the user first too. A dialog a page cannot draw or click is what stops a script
  from listing the engine at a studio of its own and moving k7 there.

**The apps load from a studio, never from the shell page.** Each app is same-origin with its backend
(relative `/api` and `/auth`, cookie sessions), so it is served by one: this phone's own by default.
The page takes the studios from the supervisor's own list, the phone's own first, and gives the view
`serviceUrl(origin, "k7")`, the mapping `UserMenu` uses — or, for the loopback one, `localServiceUrl`,
which is why the shell page and every view on the phone's own studio get `__STUDIO_PORTS__` (a
document-start script). With several studios connected, k7's settings pick
which one the apps open.

What the app views do that a browser would do on its own:

- **A link to an app with a view switches to it** instead of opening a window: `target=_blank` and
  cross-app links go to that app's view, at that URL. Only a link the *user* followed does this; a
  redirect stays in its view. Other hosts in a new window open in the browser. Every page of k7
  is same-origin with its `/auth`, so the views need no third-party cookies. Only `http`, `https`,
  `mailto` and `tel` are handed to another app; any other scheme a page names is dropped, since a
  `file:` or `intent:` URI handed out exposes or launches what it should not.
- **The microphone and the camera** go to the studio's apps only (a picture lane's take, the `live-video`
  webcam), and only once the app holds RECORD_AUDIO or CAMERA, asked for when a page first wants it (below).
- **The private CA** covers the app views too: a certificate the WebView refuses only for its unknown issuer
  is accepted when `TrustedCas` holds that certificate or its issuer. The WebView hands over the leaf
  alone, so a studio whose certificate hangs off an intermediate needs the intermediate imported.
  What `CertTrust` decides, since the WebView's own checks are skipped once it is asked: an imported
  certificate signs for others only if it is a CA (basicConstraints CA:true, and keyCertSign when
  it states a key usage); one that is not — a studio's self-signed leaf — is trusted as that exact
  certificate, by SHA-256, and never as an issuer. Either way the leaf must be in its dates and name
  the page's host in its subjectAltName (a DNS name, one leftmost `*` label at most, or the IP);
  a wrong name, an expired or not-yet-valid date, or any fault besides the unknown issuer is never
  forgiven.
- **A download is saved to the phone's Downloads.** The WebView's own download hands a URL to
  a listener that can neither read a `blob:` URL nor send the session's cookie, and the exports are
  blobs (`apps/k7/frontend/src/downloadBlob.ts`, a release's files). So the
  PAGE fetches the file: `assets/download.js`, a document-start script for the app's origin only,
  takes an `<a download>` click on a `blob:`, `data:` or same-origin URL — before the page revokes
  the blob, which `downloadBlob` does a second later — and the download listener runs the same
  script for anything it still catches. The bytes go to `Downloads.kt` over `k7Android` as base64
  chunks of 512 KiB, each acknowledged before the next, so a large file never sits whole in a
  string or a queue; it is written through `MediaStore.Downloads`, which API 29+ grants without a
  storage permission, and a toast names the file as MediaStore saved it. Any other URL still opens
  outside. The script is injected per origin, not everywhere, because the bridge drops a message
  from another origin: a click taken there would vanish.
- **Back** goes to the page first: `assets/back.js` lets a page take it (a cancelled `k7:back` event),
  then closes an open Radix dialog, sheet, popover or menu with the Escape it already answers to,
  since none of them pushes history. Only when the page used nothing does Back walk the view's
  history, and with none left it goes to the activity, where Android 12+ moves the task to the back
  and 10 and 11 finish it, which quits.
- **Insets on every side.** Android 15 draws every app edge to edge, so the views are inset by the
  system bars and the cutout on the left and right too — a landscape phone's navigation bar and
  notch are on a side — and the status-bar strip still spans the whole width.
- **The keyboard is subtracted once.** Its inset is measured from the window's bottom, and whether the
  system has already shrunk the activity above it depends on the device and the edge-to-edge mode. So
  the frame is padded only by the part of the bars and keyboard below its own bottom, measured again
  at every layout; padding by the whole inset on a phone that resizes left a keyboard-high band of the
  shell page between the view and the keyboard.

**No `cap sync`.** Gradle stages the page the way it stages the engine: `stageShell` runs the
workspace's `npm run build` and lays `dist/` out as `cap copy` would (`public/`,
`capacitor.config.json`, an empty `capacitor.plugins.json`). `settings.gradle.kts` includes
`:capacitor-android` from the root `node_modules`, so `npm ci` at the repo root has to run before
Gradle. The config is `.json` for the reason phare's is: the Capacitor CLI trips over a `.ts` one
under TypeScript 7.

**Reaching the phone's own engine from k7.** k7 is an https page, and the engine serves plain
`ws://`. The WebView, like any browser, lets an https page open `ws://127.0.0.1` but blocks
`ws://<LAN IP>` as mixed content. The engine picker probes each address an engine announces and
keeps one that answers, so a blocked one is simply not chosen. Mixed content stays off.

## A media player to the system

Android's WebView has no Media Session API at all — no `navigator.mediaSession`, no `MediaMetadata`
(crbug.com/40611412) — so a page's player gets no notification, no lock-screen controls and no
headset buttons, and with no notification the process is not held in front: Android freezes it
behind, the backend the song streams from with it. The app makes that connection.
`assets/media-session.js`, a document-start script in every view, stands in both with the
standard's shape where they are missing, wraps `navigator.mediaSession` and posts what the page sets to `StudioBridge` (`media`); `MediaPlayback.kt` holds a platform
`MediaSession` and its notification from those messages, and sends each system button back as
`media-action`, which the script hands to the page's own handler. The page code is the standard
API (`packages/ui-mini-player/docs/miniPlayer.md`) and knows nothing of Android.

| choice | why |
|---|---|
| wrap the API in the view rather than add a call to `window.k7Android` | the bar is shared and runs in browsers too; a page written against the standard is a media player in mobile Chrome and here alike |
| the platform `MediaSession` and `Notification.MediaStyle`, no androidx.media or Media3 | API 29 has all of it; Media3 is built around a `Player` the app does not have, since the sound is the page's |
| a `mediaPlayback` foreground service while it PLAYS, and only then | without one Android freezes a process whose activity is behind, sound included; paused, the service stops and the notification stays, detached, so play resumes from it |
| no `POST_NOTIFICATIONS` request | a media-session notification is exempt on Android 13+ |
| play, pause and next as notification actions too | Android 12 and older draw those rather than the session's controls |
| the notification dismissed is `stop` | it is the bar's close: the page closes its player and the session goes with it |
| the task swiped away or the activity left clears it | a detached notification would outlive the process it controls |

A foreground start can be refused when the app is behind (Android 12+); the notification then
shows without the service, and the log says so under `k7-media`. Every start of the service enters
the foreground before anything else — a placeholder notification when nothing plays — even a
start that only stops it: a `startForegroundService` left without its `startForeground` crashes
the app. So the service is stopped through a start that says so, never by `stopService`.

## Busy: the engine at work holds the process and the screen

Android freezes a process whose activity is behind, and cuts its microphone: a take, a live rig or a song playing
would stop the moment the phone locks or another app comes up. So `EngineService` is in the foreground, with a
notification, for as long as the app is **busy**, and the screen is kept on for the same while (`Busy.kt`).

| choice | why |
|---|---|
| a session PLAYING, read off the supervisor's snapshot (`sessions[].playing`) | it is the engine's own word, true whoever drives it — a browser on the LAN included; it holds the service and not the screen, since nobody is looking at the phone |
| armed, recording and a rig sounding come from k7's page (`apps/k7/frontend/src/androidBusy.ts`, a `busy` message) | the engine reports none of them to the supervisor; the page has them on every frame. A rig counts as live for 30 s after its meters last moved, so a pause between songs does not drop it |
| a microphone and a wake lock the page holds, from `assets/media-session.js` | `getUserMedia` and `navigator.wakeLock` are watched where every page runs, so the `/external` surfaces need no code for the app |
| the Wake Lock API stood in for when the WebView has none | `useStayAwake` asks for it first; the stand-in has its shape, is released on every hide as a browser's is, and keeps the window flag on |
| the screen on while a page reports anything, never for the whole activity | an idle app left open must let the phone sleep |
| `mediaPlayback\|microphone`, the microphone only with RECORD_AUDIO and while the engine's inputs or a page's microphone are open | Android 14 grants the microphone type only to a start made in front; a start refused it falls back to `mediaPlayback`, so the process stays up and only the inputs are cut |
| its own service beside the media one, not that one widened | the media service leaves the foreground on every pause by design; the engine's must not |
| POST_NOTIFICATIONS asked the first time the app is busy, in front, once per run | without it Android 13+ hides the notification and still runs the service |
| a foreground refused at `onStartCommand` tries the manifest's types last, then stops, with the service marked not held | the app may go behind between the request and the start (Android 12+); the next hold must ask a foreground start again, since a plain one is refused from behind |

A page that goes away reports nothing (`pagehide`), and what it last said is cleared, so nothing here outlives it.

## The microphone and the camera are asked when first needed

The engine starts **without waiting for RECORD_AUDIO**, and is not asked for it at start: without it the engine opens
its device output only — `EngineBridge.kt` says so with `STUDIO_ENGINE_INPUTS=0`, as on iOS. Inputs asked for without the
permission fail JUCE's whole Oboe device, output with them: the engine runs, the transport stays on its first beat,
and nothing sounds. `DeviceAccess.kt` asks when something first needs it — a page's `getUserMedia`
(`onPermissionRequest`, `PageCapture.kt`), or a track armed on the engine — with a short explanation when Android says one is due.
A permission the user has blocked is not asked again: the request is denied, the page gets a `k7:mic-denied`
or `k7:camera-denied` event, and a dialog offers the app's settings.

The camera follows the same rules, for pages only: nothing native films. One class serves both devices, since
only their wording differs and the restart below is the microphone's alone.

| choice | why |
|---|---|
| CAMERA declared with `android.hardware.camera` (and its autofocus) **not required** | the permission alone implies both features, and a device without a rear camera would be refused the app |
| a request granted whole or denied, each device asked in turn | the WebView turns a grant of part of the requested resources into a denial, so `{audio, video}` gets both or neither; the first refusal ends it |
| any resource beyond audio and video capture denies the request | the same whole-or-nothing grant: there is no way to hand a page part of what it asked |
| no page listens to either event | k7's pages already say why their `getUserMedia` failed (`useVideoRecording` names the error); the events are there for a page that wants to say more |

The engine opens its inputs once, with its device, and a stopped engine cannot be started again in the same
process (below). So a grant that comes after the engine started reaches it at the **next start**: the app says so,
and offers Restart k7 when nothing is busy (below, "Restarting k7").

## Updates come from k7.rocks, and the person installs them

A sideloaded app has no store to update it, so `Updates.kt` asks `k7.rocks/app_downloads/<channel>.json` at
every start, before the engine starts, and every 6 hours while the process lives, and fetches a newer build's APK
([docs/app-downloads.md](../../../docs/app-downloads.md) at the root). The channel and the stamp
compared are `BuildConfig.RELEASE_STAMP`, which only a CI release build carries; a checkout's build
and the e2e one check nothing.

| choice | why |
|---|---|
| downloaded on an unmetered network only, unless the person asked | the APK is hundreds of megabytes; on mobile data an automatic check waits for the next one on Wi-Fi |
| Check for updates in the home page's account menu, beside Settings | k7's page calls `checkUpdates` (StudioBridge): a check now, on any network, answered by toasts and the install dialog |
| offered, never installed on its own | an install ends the process, the engine and the backend with it; the person taps Install, in the notification or the dialog the app shows when it is in front and idle |
| refused while `Busy` says the app is at work | a take or a set must not end because a tap landed on the wrong notification |
| an APK that is not this package at a higher `versionCode` is dropped | Android would refuse it anyway; the person is never offered an install that cannot happen |
| `PackageInstaller` with `USER_ACTION_NOT_REQUIRED` | the first update still asks, and for "install unknown apps": the browser installed k7. From then on k7 is its own installer of record, and Android 12+ lets its updates through without a prompt |
| the notification opens `MainActivity`, never a receiver | Android 12+ refuses an activity started from a notification's broadcast, and the system's confirmation is one |
| the first check before the engine starts, at once | a build whose engine crashes in its first seconds would otherwise never ask |
| three quick starts in a row — none lived 30 s nor ended in a quit — leave the engine off the next run, which offers Check for updates or Start the engine (`BootGuard.kt`) | a native crash at boot kills the process before any check finishes; the engine is where one most likely comes from, and the backend and the page still run without it. The count is written before the engine starts, so the crash cannot lose it, and the run without the engine sets it back to zero, so the one after tries again |

Android checks the update's signature against the installed app's: an APK not signed with the
release key never installs, whoever uploaded it.

## Restarting k7, and opening it again during a quit

The engine starts once per process (below), and a quit ends the process with `exitProcess` after
a teardown of up to about 14 s. An activity opened in that window would live in the process about
to exit, so `MainActivity` sees the teardown under way, starts nothing, and hands over to
`Relaunch.kt`; the same path is **Restart k7**, offered when the microphone is allowed after the
engine opened without its inputs, when the supervisor asks for an engine this process already
stopped, and when the shell page's renderer keeps dying.

| choice | why |
|---|---|
| `RelaunchActivity`, in a process of its own (`:relaunch`), waits for the old pid to leave `/proc`, then opens `MainActivity` and exits | the new `MainActivity` must start in a fresh process; a start scheduled from the dying one (an alarm, a pending intent) is an activity start from the background, which Android refuses |
| started while k7 is in front, before the old activity finishes | the only moment Android lets the app open an activity |
| it draws the splash while it waits, for at most 20 s | longer than the teardown's own bounds, so a stuck teardown still ends in k7 opening |

## A renderer that dies does not take k7 with it

A WebView's renderer killed for memory, or crashed, ends the app's process unless every view it
drew claims the loss — and the engine, the backend's parent and a take in progress are in that
process. On Android every view of an app shares the one renderer, so a loss reaches the shell page
and every app view together (`RendererLoss.kt`).

| choice | why |
|---|---|
| every app view claims its loss in `onRenderProcessGone`, is destroyed, and the shell page is told with a `loadError` | a dead view can never load again; the shell's Retry makes a new one |
| the shell page's view claims its loss through a Capacitor `WebViewListener`, and the activity is made again | it is the bridge's view, which only a new activity replaces; `recreate()` is no quit, so the engine and the backend keep running |
| the apps lost before the activity was made again say so at their next `show`, once | the new shell page would otherwise reload k7 straight into the page that ran out of memory; the person's Retry is what loads it |
| the shell made again more than three times in a minute is offered Restart k7 instead | the shell page itself dying would loop forever |
| the throwaway view behind `window.open` (`WindowProbe.kt`) claims its loss too | it shares the renderer, and one view left unclaimed is enough to end the process |

## No sign-in in the app

The phone opens its own studio only, as the local user: the app has no sign-in to a server, no
pairing and no studio to connect to, so no view navigates to `/auth/login` and the app answers no
`<app id>://` link. The shell page lists no
studio forms; what it keeps is the engine's state, the LAN pairing and the private CA below.

**The account is k7's page's**, as in a browser, not a native bar over the views: a bar over every
page is height taken from all of them. Who is signed in still reaches the native half
(`assets/account.js`), asked on every load and whenever the page comes back to the front: nobody
means back to the local user.

## A library in the app, not a process it spawns

JUCE's Oboe backend asks Android's `AudioManager` for the device's rate and buffer through JNI, and
JUCE as a whole wants `Java.initialiseJUCE(context)` before anything else. A bare executable has no
JavaVM to give it. So on Android the `k7-runtime` target is a **shared library**, `libk7runtime.so`,
built from the same `STUDIO_SERVER_SOURCES` as the desktop binary (`server/CMakeLists.txt`) plus `server/android/EngineJni.cpp`, and
`server/main.cpp` — the CLI — is not built at all. `serve()` is the whole server on every platform;
the desktop `main()` calls it once its CLI modes have declined.

`com.rmsl.juce.Java` in the app is JUCE's bootstrap class under JUCE's own name, because the
engine's `JNI_OnLoad` (JUCE's) registers `initialiseJUCE` on exactly that class. JUCE's copy loads a
library called `juce_jni`; the app's loads `k7runtime`, which is where JUCE lives here.

## JUCE's message thread is a thread with a prepared Looper

On Android, JUCE posts every message through an `android.os.Handler` it creates with no argument —
which binds to the Looper of **the thread that first creates the MessageManager**. That decides the
whole shape:

- `EngineBridge` starts a plain `Thread` that calls `Looper.prepare()` and then `nativeServe`. The
  `EngineHost` constructor is the first JUCE call on it, so that thread becomes the message thread.
  Not a `HandlerThread`: a runnable posted to one already runs inside its `loop()`, and the engine
  looping it again would nest two loops that one quit then tears down together.
- `EngineHost::run()` calls `Looper.loop()`, which blocks and dispatches exactly as the desktop's
  `runDispatchLoopUntil` loop does. The rest of `serve()` is unchanged: its locals live on that
  stack for as long as the engine runs.
- `EngineHost::quit()` posts `quitSafely()` to that Looper, from its own thread. JUCE's
  `stopDispatchLoop()` would instead finish the app's Activity.
- **Once per process.** After the quit, JUCE is still bound to a Looper that cannot loop again, so an
  engine is restarted by a new process. `MainActivity` exits the process when it finishes, and a later start the supervisor asks for offers Restart k7 instead (`Relaunch.kt`).
  So a start must not fail: the supervisor hands the app a port it has just BOUND itself, the first
  free one of the usual ten (another app's engine — k7 and k7 e2e side by side — holds 8765), and a
  connect cannot stand in for that test, since another app's listener refuses this one's loopback.
  It adopts a running engine only when this same app process started it, on the port it recorded.

Teardown after the quit posts a few messages to the dead Looper; Android logs "sending message to a
Handler on a dead thread" and drops them. That is the same as a desktop message posted after its
dispatch loop stopped: never delivered, and nothing waits for it.

JUCE forces `JUCE_MODAL_LOOPS_PERMITTED` to 0 on Android, so `runDispatchLoopUntil` does not exist
there. Every other caller of it is a CLI mode, which Android never runs, so they all go through
`pumpMessages()` (`engine/MessagePump.h`), a no-op on Android.

### JUCE's calls into the app's views are not made from the engine

JUCE takes its message thread for Android's main thread, and the engine's is not: a JUCE call that touches
a view or the window throws `CalledFromWrongThreadException` from there. Tracktion makes one on every
record roll — `ScreenSaverDefeater` sets the window's keep-screen-on flag — and only once JUCE has seen the
activity start after the engine did, so a roll right after a launch passes and one after a trip to the
background does not.

| choice | why |
|---|---|
| `ScreenSaverDefeater` does nothing on a phone (`engine/patches/tracktion.cmake`), iOS included | the screen is the app's (`Busy.kt` sets the same flag through `MainActivity`, and `Busy.swift` sets `isIdleTimerDisabled` on the iPhone): the engine clearing the flag at a stop would turn off what a page still asks for; a computer keeps Tracktion's defeater, where nothing else holds the display |
| a Java exception a message leaves pending is logged, cleared, and `run()` loops again | it unwinds `Looper.loop()` with the exception still pending, so the engine stops serving and its teardown's first JNI call aborts the app on it; the engine cannot be restarted in the same process, and one failed message does not justify that |

## The supervisor is a program the app runs; the engine is a library it loads

The supervisor needs no Java — no JUCE, no audio — and is built to be a child driven over NDJSON on
stdio, so it stays exactly that. The app packages it as `libk7supervisor.so`: only files named like
a library reach the native library directory, the one place Android lets an app execute its own
binary, and `useLegacyPackaging` has the installer extract them as real files. `SupervisorHost.kt`
is then what `k7-local`'s host (`apps/k7/backend-rs/docs/supervisorHost.md`) is to the tray's front — it starts the program, reads its frames, sends its commands,
and closes stdin to quit it.

Only the ENGINE changes hands. On a computer the supervisor spawns it and kills its process tree; here
it lives in the app, so `EngineProcess.cpp` asks instead:

- **start** emits an `engine` frame carrying the id, token and port it would have put in the
  child's environment, then waits for the port to answer. The app sets that environment and serves.
  The pidfile names the supervisor's PARENT, which is the process the engine is a thread of.
- **stop** emits the request and never kills anything: that pid is the app's own.
- An engine already serving is **adopted**, as on a computer — a supervisor the app restarted finds it.

**One supervisor per process, not per activity.** Android recreates an activity on its own — an
overlay change is enough, and `configChanges` does not cover it — and a second supervisor is refused
by the host lock while the first keeps drawing into the dead activity. So `SupervisorHost` is an
`object`, like `EngineBridge`: it starts once, keeps the last snapshot, and whichever activity is on
screen listens.

**A supervisor that dies unasked is relaunched**, on `SupervisorHost`'s backoff — 1 s doubling to
30 s, back to 1 s after a run of 20 s — and its last snapshot is marked down meanwhile, so the
pages say why rather than show a stale status. Without it the phone drops off every studio until the
app restarts; the engine keeps serving through the gap and the new supervisor adopts it. It never
gives up: unlike the backend, nothing on the phone would retry it.

**No instrument is downloaded until the user asks**, in the Downloads section of k7's settings —
the engine page's own section, through this app's bridge (`engine/k7-supervisor/docs/instruments.md`).
Every instrument together is close to 3 GB, the piano alone 750 MB, and a phone is often on a
metered network: a first launch that fetched them all would spend a data plan on instruments
nobody chose.

Two things a phone answers differently from a desktop:

- **Trusted roots.** Android keeps one file per CA, the updatable conscrypt module's set on Android
  14+ before the system image's, each a PEM followed by a text dump. `TrustRoots.cpp` reads the
  blocks out of them.
- **The machine's name.** `gethostname()` is "localhost" on every phone, so the app passes the name
  the user gave the device, else its model, as `STUDIO_MACHINE_NAME`, and that is what the picker
  shows.

No ONNX Runtime ships for Android, so the supervisor builds without stem separation: the job says
what is missing, as it does on any platform without the library.

**A private CA is the user's to add, in the app — and optional.** A git host with a public certificate
needs nothing: Android trusts Let's Encrypt as every
OS does, so the section is folded away unless a CA of the user's is already trusted, and says when
one is worth adding. A git host behind a private CA is exactly the case the
desktop answers from the OS store, and on Android the certificates a user installs in Settings are
out of an app's reach. So the app takes the CA itself, through the document picker — PEM or DER,
one or a bundle, parsed by the platform's X.509 factory so a file that is not a certificate is
refused rather than stored — and keeps them as one PEM file handed to the supervisor as
`STUDIO_EXTRA_CA_CERTS`, and to the front as `K7_EXTRA_CA_CERTS` (its pairing, the shares: every
outbound call, `apps/k7/backend-rs/docs/front.md`), which reads it once, at its start, as the next app start does.
The supervisor reads its roots once, at start, so a change restarts it:
the running engine is adopted, never stopped, and only the CURRENT supervisor's `engine` requests
are obeyed, since the old one's last words — stop the engine — may still be in its pipe.

## The buffer: two bursts at least, and more when the phone underruns

JUCE's Oboe device calls back one hardware **burst** at a time — 96, 192 or 240 frames on most
phones — and the buffer size the engine sets is only how many frames Oboe keeps queued ahead of
that. The desktop's 256 (`docs/audioDevice.md`) snaps to the nearest whole burst, which on a
192- or 240-frame phone is ONE: no slack at all, so every callback that runs late — a core
migration, a frequency step, one heavy block of the graph — is an underrun. It is heard as clicks
and crackle laid over playback that otherwise runs on time, not as a stutter, which is what makes
it read like a DSP bug rather than a timing one. The emulator hides it: its device hands out 960.

So on Android the engine asks for **at least two bursts**, and `checkXRuns` reads the stream's
underrun count once a second: when it moved, the buffer goes up ONE size and the log says so
(`[audio] N underruns at X frames — buffer now Y`). It stops at 40 ms, JUCE's own default for a
low-latency Android device; past that the log says the cap was reached, then counts every
further underrun (`… more underrun(s) at X frames, the cap`), at most once a second. At the cap it
asks Oboe for nothing more: Oboe answers "which buffer sizes?" by OPENING a throwaway stream, on the
output the music plays through, and doing that once a second is itself a click a second. Growing
reopens the stream, a short gap, bounded by the cap — against a crackle that would otherwise never
stop. It never shrinks back: a phone that underran once will again.

**No steady load.** On Android, Tracktion wraps each audio callback in a `ScopedSteadyLoad` that
busy-waits until 80 % of the callback's period has gone by, so the core's clock never drops between
callbacks. That suits a low-latency stream of a few milliseconds. A phone's shared output gives this
engine 20 to 40 ms, so the spin held a core at real-time priority while the graph needed a few
percent of it, and handing each buffer over at 80 % of its period left too little slack: measured on
a Nothing Phone (4a) Pro, the stream underran once a second and the speaker crackled. Without it the
busiest thread falls from about 80 % of a core to under 15 %, and the stream underruns a couple of
times a minute. `patches/tracktion.cmake` sets its level to zero at fetch time, so the busy-wait has nothing to wait for.

To check on a phone: `adb logcat -s k7-runtime` shows the buffer at open and every step.

## Build choices, and what forced each

| choice | why |
|---|---|
| The presets build, Gradle stages | AGP's `externalNativeBuild` would configure second CMake trees and fetch Tracktion into one. `app/build.gradle.kts` runs the engine's `scripts/build.mjs android-arm64` and `engine/scripts/supervisorBuild.mjs` with `STUDIO_SUPERVISOR_PRESET=android-arm64`, against the SDK's NDK, and copies both in. |
| API 29 | JUCE's Android font code calls `AFontMatcher`, which is 29+. AAudio, which Oboe prefers, only needs 26. |
| arm64-v8a only | every phone worth playing on; a second ABI doubles a 35 MB library for nothing. |
| No VST3 / LADSPA | there is nothing to host and no window for an editor. `loadVstPreset` answers false, as it does for a built-in. |
| Tracktion populated, not built from its root | its root always adds its examples, and `EngineInPluginDemo` depends on a VST3 target that does not exist here, so configure fails. `modules/juce` and `modules` are added by hand instead. |
| `JUCE_LOG_ASSERTIONS=0` | JUCE logs assertions in RELEASE on Android, which compiles Tracktion's `jassert (hasPrefetched)` against a member that only exists in debug. |
| `-Dtls_model(x)=unused` on `tracktion_graph.cpp` | the rpmalloc it compiles in pins its thread heap to initial-exec TLS, and Android's loader refuses that in a library opened by `System.loadLibrary` — the app dies loading the engine. The rewrite turns the attribute into `unused`. A compile OPTION, because CMake drops function-style definitions silently. `llvm-readelf -d libk7runtime.so` must not show `STATIC_TLS`. |
| `CMAKE_POLICY_VERSION_MINIMUM=3.5` | Oboe's own `CMakeLists.txt`, which JUCE adds on Android, declares a minimum CMake 4 refuses. |

## On a Windows machine

The APK builds on Windows too, for a phone on the desk without a round trip through CI: NDK
`29.0.14206865` from the SDK manager, Android Studio's JDK 21 as `JAVA_HOME` (Gradle refuses a
newer one), and Gradle run from a Visual Studio developer prompt (`vcvars64`) — JUCE builds its
`juceaide` for the HOST while cross-compiling, and under Ninja that needs `cl.exe` on the path. A
local build is signed with the debug key, so it installs as the `e2e` type beside the shipped app,
not over it: `gradlew :app:assembleE2e`. The scripts it runs keep to `/` in what they write.

## Release builds

No CI job builds the APK: a developer builds the RELEASE type, not debuggable, on a machine with the SDK, JDK and Visual Studio above, signed with the release key from the environment; a debug build is for a developer's own phone, and replacing one with the
other needs the app uninstalled first. The `e2e` build type is the exception: its own id and port put it
beside either, which is what the functional suite installs (`e2e/docs/e2e.md`).

## Logs

Everything the engine writes goes to stderr, which Android discards. `nativeServe` pipes stderr into
logcat under the tag `k7-runtime`; JUCE's own lines appear under `JUCE`. On the emulator the pipe
also catches the GL driver's chatter, which is noise.

The Rust front's lines (`files |`, `sync |`, `tunnel |`, `[registry]`, its panics) are under `k7-front`: `k7-embed`
pipes the process's stdout and stderr into logcat when the front starts, and the engine's pipe takes stderr once it
serves, so the front's stderr lines are `k7-runtime`'s from then on (`apps/k7/backend-rs/docs/embed.md`).
`adb logcat -s k7-front k7-runtime k7-backend` reads the backend's side.

## Git on a phone

libgit2 and zlib are compiled into `libk7_embed.so` by the NDK's clang (`buildFront`), with k7's own transports and no OpenSSL or libssh2 (`apps/k7/backend-rs/docs/git.md`). The secrets are kept in a private file of the app, since Android has no keychain the front reaches. The checklist, on a phone with a repository host and a second machine:

| check | do | look for |
|---|---|---|
| clone over HTTPS | Share a project, paste an `https://` repository URL, sign in with a token | the workspace appears with its files; the token is kept and a second fetch asks nothing |
| clone over SSH | the same with an `ssh://` URL and a key made or imported in the app | an unknown host's fingerprint is shown and must be accepted once; a changed key is refused |
| commit | edit a song, open the project's history, commit | one entry named for the device; a commit with no change is refused |
| push | commit, then push | the remote has the commit; a remote ahead answers behind and nothing is lost |
| pull with a conflict | change the same lines here and on another machine, push there, pull here | the conflict is listed, both sides are shown, choosing one finishes the merge as one history entry |
| a large file | add a 20 MB WAV, commit, push, then clone it on another device | the repository holds a pointer, the host's LFS holds the file, the clone plays offline after its pull |
| no second TLS or SSH stack | `strings` on the staged library | no OpenSSL and no libssh2 in it |

## What it does not have yet

- **The standalone app has not been built and run as one APK yet.** Its backend and file service
  were run on the emulator as an app, with `BackendHost.kt`'s layout and environment (below); the
  APK itself, and the view on the local studio are still to drive.
- **No ffmpeg on the phone**: waveforms are decoded by WebAssembly instead, and an mp4 or a .mov is probed from its boxes
  (`packages/audio-analysis/docs/analysis.md`); what else needs them — transcoding — answers as it does on a machine without them.
- **The tunnel and WebRTC are untested on a phone.** They are the code a computer runs, built unchanged; no
  studio has driven a phone through them yet.
- **No external plugins, no plugin scan.** An offline render or freeze is the engine's own, asked by the supervisor's `render` job over the engine's socket (`protocol.md`), and `Busy.kt` counts it as busy.
- **The busy service and the microphone flow have not been driven on a phone yet**: a take with the screen off,
  the microphone type refused from behind on Android 14, the notification permission and the blocked-microphone
  dialog are each still to check.
- **The camera is compiled, not driven**: a picture lane's take, the `live-video` webcam, a request for both devices
  with one refused, and the blocked-camera dialog each need a phone. A camera holds nothing in front: a page that
  captures video only is not busy (`media-session.js` counts audio tracks), so the screen may sleep and Android cuts
  the camera of an app behind.
- **The media session has not been driven on a phone yet**: background playback in a hidden view,
  the notification on Android 12 and 13+, and a headset's buttons are each still to check.
- **The app views have not been driven on a phone yet.** The routing rules, the home page's account
  control, the status-bar colour and k7's engine section above are the design; each needs checking.
- **Updates have not been driven on a phone yet**: the first install's "unknown apps" prompt, the silent
  path on Android 12+ once k7 is its own installer, and Busy's refusal each need a phone and two published builds.
- **Downloads, Back and the start screen are compiled, not driven.** The in-page fetch of a revoked
  blob, a file of hundreds of megabytes through the chunks, Back closing a Radix sheet, the
  landscape insets and a failed start's Retry each need a phone.

## What was measured

On an Android 15 arm64 emulator on an Apple Silicon Mac, with `scripts/measure-broadcast.mjs`
through `adb forward`:

| | measured |
|---|---|
| library load, JUCE init, port bound | 60 ms after launch |
| audio device | Oboe, 48 kHz, 1 in / 2 out, 960-frame buffer (the emulator's own) |
| a 4OSC note over 25 s | every frame carries signal, 0 dropped, 0 undecodable |
| jitter spread | 4.6–6.2 ms |
| realtime ratio | **0.88, steady** |
| Stop with a project loaded | the engine tears down and the process exits cleanly |
| supervisor start | 145 trusted roots read; engine started by the app with the supervisor's identity |
| connecting a studio (a stand-in registry through `adb reverse`) | hello answered, a heartbeat every 5 s carrying id, LAN addresses, name and performance |
| Quit | deregister sent, pidfile and host lock removed, both processes gone |
| an HTTPS studio whose certificate a private CA signed | refused; after importing that CA (DER) 146 roots, the engine adopted, connected and heartbeating; after forgetting it 145 and refused again — one engine start throughout |
| importing a file that is not a certificate | refused with a message; nothing stored, nothing restarted |
| `--probe https://app.priv.k7.rocks` with Android's CAs only | an k7 registry: nothing to import for a publicly certified studio |

The realtime ratio is the emulator's audio device consuming 12 % less than wall-clock time at a
nominal 48 kHz: steady from 8 s to 25 s, with nothing dropped, so the engine keeps up with the
device and the device is what runs slow. It says nothing about a phone. The number that matters —
latency and the ratio on real hardware with a real audio path — is still to be measured.
