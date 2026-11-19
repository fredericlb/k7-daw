# The functional suite

One set of specs, `specs/app/`, run on one of three **targets**:

| target | what it drives | command |
|---|---|---|
| `web` | a **running** k7 in Playwright's Chromium: the instance `npm run dev` serves, a device on loopback (`http://127.0.0.1:5223`), named by `E2E_BASE_URL` | `npm run test:e2e` |
| `android` | the app on a phone — its `e2e` build, built and installed by the run itself — through the app's own WebView | `npm run test:e2e:android` |
| `ios` | the app on the iOS simulator, its studio driven in the host's WebKit | `npm run test:e2e:ios` |

```bash
npm run e2e:install --workspace @studio/e2e   # once, for the web target: Playwright's Chromium
npm run test:e2e                              # every spec, on the web
npm run test:e2e -- specs/app/transport.spec.ts

npm run test:e2e:android
npm run test:e2e:ios                          # the app built in ~/.cache/k7-ios-dd (engine/k7-runtime/docs/ios.md)
```

Neither is part of `npm run test`, whose workspaces run vitest: both need an engine, which a unit run
must never need. A failed spec keeps its trace under `e2e/test-results/`; the HTML report is
`e2e/report/`. Requests answered 4xx and console errors do not fail a
spec: they are listed on the report as `noted` annotations, since some 4xx are the app asking whether
a file exists. A 5xx fails one.

## One `test`, bound to a target when the suite loads

Specs import `test` from `src/target.ts`, never from `@playwright/test`, and drive a `k7` fixture:
`k7.open(path)` for a page, `k7.landed(from, at)` for the page a click led to, `k7.files()` for the
file service. They never call `page.goto`, because the two targets do not agree on what the page is: on the web it
is Playwright's own, on the instance's origin; on the phone it is the k7 view among the app's WebViews,
on the phone's backend origin, reached over adb.

The target is chosen by `E2E_TARGET`, which `playwright.android.config.ts` sets, rather than by a
Playwright option: a fixture's dependencies are fixed when it is declared, so one `test` serving both
would make the phone run start Chromium too.

## The ios target drives the app's studio, not its WKWebView

Playwright reaches no WKWebView, so the page is the host's WebKit on the simulator's loopback, which it shares:
the studio, the engine and the supervisor are the app's own, the screen is not. What the target decides:

| choice | why |
|---|---|
| the booted simulator on the newest iOS runtime, unless `E2E_IOS_DEVICE` names one (the Mac keeps `k7 iPhone (iOS 27)`, an iPhone 17 on iOS 27.2) | a new iOS enforces at launch what an older one only warns of: iOS 27 stops an app without the scene lifecycle, which iOS 26.5 starts |
| the app relaunched on port 18799 through `K7_STUDIO_PORT` | the tray holds 18789 on the same loopback, and the port is read once, at launch |
| the app's engine on port 18765, written to its `engine-host.json` before every launch, and the app terminated when the suite ends | the simulator shares the Mac's loopback and the app binds 8765, the user's own engine's port; no `SIMCTL_CHILD_*` variable moves it, the file does |
| WebKit with the iPhone's size and agent, but not as a mobile browser | mobile WebKit has no mouse wheel, which the specs zoom with |
| `open` waits for the page to settle | WebKit reports a fetch a reload cut short as a page error, which Chromium does not |
| `E2E_BASE_URL` is the app's origin | the verb runner is node's and takes a whole URL |
| a spec that reads a phone's popover clicks the editor again before its next shortcut (`editing.spec.ts`) | WebKit focuses no button it clicks, so the keys land on the page's body, and Ctrl+Z and Ctrl+Y off the editor are the project journal's |

Specs that skip here, each with its reason in the spec (`engine/k7-runtime/docs/ios.md`): the simulator has no camera, so
the picture lane's take skips. A member that needs a program is not a skip but an answer: `noPrograms.spec.ts` asserts what
each one says on iOS, since the phone starts no ffmpeg, whisper or browser.

What still fails on the simulator, none for want of a program: `rack-demo` and `synth-demo`, whose
`/module-catalogue` answers 503 (cause not found; the memory budget's decline is the lead).

`stems.spec.ts` cuts a clip on the phone and runs only with `E2E_STEM_MODEL`, the HT-Demucs `.onnx`: the target clones it into
the supervisor's copies of server folders (`.k7/remote/models`) and names that folder `$STUDIO_MODELS`, because a cut reads nothing
outside those copies and the front's cut answer lists no models root there. Its memory-floor test runs when the run sets
`SIMCTL_CHILD_STUDIO_SEPARATION_MIN_MEMORY_MB`, its behind-another-app test when `E2E_STEM_BEHIND` gives the clip's seconds.

`flows.spec.ts` drives the app's native flows (a download, a private CA, the microphone's refusal, a killed page) through
debug-only hooks, `K7_TEST_HOOKS=1` handed at launch, and reads each result from the app's `[host]` log line
(`engine/k7-runtime/docs/ios.md`, "Proven on the simulator"). It relaunches the app itself for the microphone and the killed page, and
revokes the simulator's microphone grant, which it restores.

## The web target signs in without the identity provider

The instance signs its session cookie with its `SESSION_SECRET`, and that secret is in this machine's
`env.yaml`. `src/session.ts` signs one the same way, with `sessionCookieHeader` from
`src/sessionCookie.ts` — the helper that exists so tests do not hand-roll the encoding. So there is no
password to store and no login page to script, and the suite cannot sign in anywhere whose secret is
not on disk here: it is a local tool by construction.

It signs in as a dedicated account, `sub: "e2e"`. The file hub gives that account a workspace of
its own, seeded with the `Examples` projects, so a spec that writes never touches a person's work.
The account is not an admin: a spec that needs an admin's route skips, or checks that it is refused.

## The android target installs a build of its own, beside the shipped app

The run drives the app's WebView, and Playwright's `_android` reaches only a DEBUGGABLE app's: Capacitor
turns WebView debugging on for exactly those builds. The APK CI ships is the release type, and a
debuggable build of the same id cannot replace it without an uninstall, which deletes the phone's
workspaces. So the run builds `e2e`, a build type of its own in `engine/k7-android/app/build.gradle.kts`:

| what differs | why |
|---|---|
| the id `se.frediver.k7.daw.e2e` | installed beside the shipped app, with its own data, and removed without touching it |
| the front on port 18799, not standalone.json's | every app of a phone shares one loopback, and the shipped app may be running. The engine needs no such care: its port is the first of a range |
| the label "k7 e2e" | the two launcher icons are told apart |
| `E2E_BASE_URL` is the phone's origin, and a device target's verb runner refuses a relative base without one; `android-check.sh` asserts the config sets it | the verb runner is node's and takes a whole URL, and the web default sends every verb to the private instance, which answers 502 |

It signs in as the local user only.

`src/androidBuild.ts`, the global setup, runs Gradle's `installE2e`, which builds what changed and
installs over adb — the engine's C++ is the cold part, built at half the cores, since a full engine
build beside a dev session's own watcher has frozen a Mac. `ANDROID_HOME` defaults to where Android
Studio puts the SDK. `E2E_ANDROID_BUILD=0` skips the build and runs what is installed, and
`ANDROID_SERIAL` picks a device when several are attached.

The worker then starts the app, forwards the backend's port with `adb forward` — the run seeds the
fixtures over HTTP as on the other targets — and takes the page of the k7 view among the process's
WebViews, the one on the backend's origin. The fixtures go into the e2e app's local user, whose
data is its own.

Two things the run does that a person does once by hand. It writes the phone's engine into the
account's preferences, since a project page picks no engine on its own and a fresh install has
none remembered. And a spec's `page.screenshot` is the phone's screen, taken over adb: a WebView
never answers DevTools' capture, and the k7 view is what the screen shows.

## Feature projects

`fixtures/` holds one small project per feature, and `fixtures/README.md` is the table of which is
which. They are written into a workspace of their own, `E2E`, before every run. Writing goes over
HTTP, through the same routes the browser uses: the workspace is removed and recreated with
`/workspaces/remove` and `/workspaces`, never deleted on disk, because the capture would see the folder vanish as an edit.

The service validates nothing on write, so whether a fixture is a valid project is a test of its
own (`specs/app/fixtures.spec.ts`), separate from what the screen does with it
(`specs/app/features.spec.ts`).

A spec whose subject is not one project builds what it needs at run time, in a workspace of its
own: the `@k7-core` library module by module (`library.spec.ts`), the app's routes, the home page's
workspace dialogs (`workspace-ui.spec.ts`),
recording, the frozen chains and their bench, and the surfaces each archetype opens on.

`git-sync.spec.ts` connects a workspace to a bare repository the spec makes with the `git` binary, commits and pushes, clones into a second
workspace, edits the same key on both sides and resolves the conflict. The remote is a folder on this machine, so the spec is the web
target's alone and needs the front started with `K7_GIT_ALLOW_FILE` (`apps/k7/backend-rs/docs/git.md`).

A spec runs on both targets unless what it checks exists on one only, and then it says so in its
`test.skip`: a signed-out visitor or Chromium's fake microphone is the web's.

A spec that has found a bug keeps asserting the right behaviour under `test.fail`, with the bug in
its reason: the run stays green, and turns red the day the bug is fixed, which is the reminder to
drop the mark.

## Play is pressed once, when it is offered

"engine connected" shows before the project has reached the engine: the files still cross the tunnel,
and the engine replaces its edit when they land, which drops a transport already rolling. The project bar
therefore names the button "loading the project", disabled, until the file service says the project
is playable (`apps/k7/frontend/docs/shell.md`). `src/transport.ts` waits for the button called Play
and presses it once: a Play that is lost again is a failure, not something to press through.

## A spec waits for the thing it waits for

No spec sleeps to let something happen. It waits on the event that says it did: a locator's state, a
response (`page.waitForResponse`), the transport's clock reading more beats (`rolls` in
`src/transport.ts`), a meter's title saying a record sounds, a frame on a socket. What no socket
announces is asked again with `until` (`src/until.ts`), on `expect.poll`'s pace, which starts at 50 ms:
a sleep is a guess, paid in full on every run where the thing came early, and too short on a loaded
machine where it came late.

A fixed window stays only where time is what is checked, and its comment says so: a stopped clock
holding for a second (`transport.spec.ts`), a ticket's minute (`cameras.spec.ts`), a relay sending
nothing more (`Relay.rest`), an engine sent no dry run, a song or a workspace staying removed. An
absence has no event to wait on.

## What a web run needs from the machine

- **On the Windows machine, a run is Windows'**: from the verify worktree under Git Bash, with
  `E2E_ENV_FILE` naming the checkout's `env.yaml`. WSL's Chromium lacks the system libraries it
  loads, and the phone's APK builds only from a Visual Studio prompt, so the phone run takes
  `E2E_ANDROID_BUILD=0` after `gradlew :app:installE2e` there (`engine/k7-runtime/docs/android.md`).

- **The dev session up** (`npm run dev`).
- **An engine listed at the instance.** On a project, the picker lists only what the registry knows,
  so the supervisor must name the instance in its config — `~/.k7/engine-host.json`:
  `{ "registryUrls": ["https://app.priv.k7.rocks"] }`. Without it every project spec stops at
  "No engines found". `node e2e/scripts/engines.ts` prints what the e2e account sees. Against the
  dev session's own registry, that is `{"registryUrls": ["http://localhost:8789"]}`.
- **A dev `env.yaml`** at the root, with `STUDIO_DEV_AUTH: "1"`, a `SESSION_SECRET`, an
  `AUTH_ADMIN_GROUPS`, a `STUDIO_FILES_ROOT` and `STUDIO_LIBRARY_SEED: "k7-core=<checkout>/k7-core"`.
  The files root sits under the engine's state (`~/.k7`) or in `$STUDIO_ENGINE_ROOTS`: the engine
  refuses every clip and preset path outside its roots.
- **A library**: none to create, the bundled seed (`STUDIO_LIBRARY_SEED`) is the account's `k7-core`.
- **`E2E_BASE_URL=http://localhost:5223`** when the private instance's proxy does not answer.

What fails on a dev session whatever the code: Vite's HMR socket dials `ws://localhost:443` (it is
configured for the TLS proxy) and a page reports "WebSocket closed without opened"; and Tilt's
`native` build fails with LNK1168 while another run still holds `k7-supervisor.exe`.
- **The engine runs on a real device, and the speakers stay quiet on the macOS bed.** The transport spec
  plays a few seconds of `synth-demo` through the engine's output device. `e2e-macos.sh` starts its engines
  with `STUDIO_ENGINE_SILENT=1`, which writes silence after every meter and tap
  (`engine/k7-runtime/docs/audioDevice.md`); `K7_BED_SOUND=1` lets a run be heard. Chromium runs with
  `--mute-audio`. A run against the dev session plays through that session's own engine, with sound.
- **No camera and no microphone.** Chromium runs with fake media devices, granted without a prompt
  (`--use-fake-ui-for-media-stream --use-fake-device-for-media-stream`, in `playwright.config.ts`),
  so the recording and webcam specs need no hardware. An audio lane records the ENGINE's input,
  which no browser flag fakes: that spec takes whatever the engine's input channel 1 hears.

**The machine must stay awake.** A laptop that sleeps mid-run freezes the app and the engine, and
on waking every timer fires at once: the spec then fails on a closed page or a lost engine that are
nothing but the sleep. Run on AC power with the lid open, under `caffeinate -dims`, which does not
hold off a closed lid on battery.

One worker runs the specs in order: they share one account's workspace and one engine, which serves
one account at a time.

## The Mac's bed

`engine/scripts/ci/e2e-macos.sh` runs the web suite, the conformance suite or both on the Mac, against a dev session
of its own; an agent starts it through `scripts/agent/bed.sh` (below). Its results are the HTML reports, Playwright's
JSON, the failed specs' traces, every process's log and `summary.txt` (the counts) with `timings.txt`. A red run is a
failed spec or a bed that did not come up. With the builds warm, both suites take about seven minutes, the web
suite's length, since they run side by side; a cold tree first compiles the engine for twelve.

| choice | why |
|---|---|
| a session of its own, not the Windows one | a suite no longer waits for another agent's run, a Tilt restart or a merge rebuilding the session it drives |
| the front and Vite started as `npm run dev` starts them (`dev-front.mjs`), on ports 28789 and 25223, all on `127.0.0.1` | the same session as Windows', so a difference is the platform's; the Mac may run a dev session or a tray of its own on the usual ports, and its application firewall drops a LAN connection to an unsigned binary |
| the command-line engine (`k7-supervisor --config`), listed at the bed's front over loopback, on 28765 | an engine heartbeating from loopback is every local account's (`apps/k7/backend-rs/docs/engine/ownership.md`); a tray would hold 18789 and `~/.k7`, the Mac's own |
| `HOME` under `~/k7-trees/e2e-macos-bed`, wiped at every run, an env.yaml with a new secret, no instrument downloaded | nothing of the Mac's own `~/.k7` or `~/.studio` is read or written, and the bed is paired with no server: the specs that publish or sync stay away from production |
| the release front built into `~/.cache/k7-macbed-<tree name>` (`--target-dir`, handed to `dev-front.mjs` as `K7_FRONT_TARGET_DIR`) | a `target` inside the worktree makes `gate.sh` refuse the tree, and two worktrees must not share one build folder |
| `STUDIO_HOME` given to the engine, `STUDIO_PORTS=k7=28789` to every process | macOS gives the engine the account's home whatever `HOME` says, and the supervisor reads any loopback registry as k7's port, 8789 unless that names another. Tracktion's `~/Library/Studio/Settings.xml` (the audio device) stays the Mac's, shared with any other engine there |
| one suite takes slot 1 on the ports above and `~/k7-trees/e2e-macos-bed` (`E2E_BED` names another); `both` takes slots 1 and 2 together, slot 2 a hundred ports higher in `~/k7-trees/e2e-macos-bed-2`, and runs the web suite on the first and the conformance suite on the second at the same time; the engine build, which empties a folder of the shared dependencies, is taken in turn | the suites run one after the other cost the conformance suite's three minutes on every full bed; on one bed they would clash, since a device has one account and the conformance service lane holds its file service and served project; a slot whose ports already listen, or whose lock's owner is gone, is judged by that, not by its lock alone |
| the two beds share one `env.yaml` holding what they share (the secret, the device switches); each bed's folders (`STUDIO_FILES_ROOT`, `STUDIO_MEDIATEK`, `K7_FRONT_CACHE`) reach its processes as variables, and the second bed logs under `conformance-bed/logs`, judged by `bedCounters.mjs` as a conformance run | the backend finds `env.yaml` by walking up from its folder and a key there beats the environment, so a tree holds one; the counters read one front's log |
| Playwright stops a run at its tenth failure (`--max-failures`, `E2E_MAX_FAILURES`, 0 for none) | a branch that breaks something basic otherwise waits out every test's timeout: over an hour for nothing a short run does not already say |
| the bed starts the suites only once the front's relay answers something other than 503, and a stop is done when the bed's four ports are free | the front answers `/auth/me` before its relay runs, so a spec started in that gap meets 503 and then 409 on every first call; a bed that cannot get a relay fails in minutes with the front's log, not after a whole suite of 409s |
| the engine, the front and `node_modules` kept warm in `~/k7-trees/tree-e2e-macos`, through `ciTree.mjs` | a run after a small push builds only what changed |
| Playwright's Chromium and the render page (`k7-picture/page.mjs`) built at every run, for the conformance suite as well as the web one | `picture render` drives that Chromium with that page, and fails without either |

The speakers play: the transport spec sounds through the Mac's default output.

## The bed dispatcher: `scripts/agent/bed.sh`

An agent never runs `e2e-macos.sh` itself: `scripts/agent/bed.sh` waits for the Mac's bed slot and runs it there.
It takes the macOS script's parameters as environment (`E2E_SUITE`, `E2E_FILTER`, `K7_BED_SOUND`), run from
the branch's worktree. The front runs one mode, every piece native: a bed names no switch.

```bash
E2E_SUITE=conformance E2E_FILTER="serviceFiles" bash scripts/agent/bed.sh
bash scripts/agent/bed.sh --dry-run           # says what it would do, starts nothing
```

The results of a run are under `~/.cache/k7-bed/<worktree>/mac/e2e-out` (`K7_BED_OUT` moves the base):
`summary.txt`, `timings.txt`, `web.json`, `conformance.json`, the reports and `logs/` with `front.log`.
A branch's run is judged against main's, run the same way, with `scripts/agent/sameFailures.mjs`.

Every bed script ends by judging the native counters in `front.log` with `scripts/agent/bedCounters.mjs` (the engine
link, media, the runtime line and the shed; the jobs and the exports when conformance ran), and adds what it read to
`summary.txt`; the exit status is the suites' and that judge's. At its end a bed prints `bedSummary.mjs`'s summary: the
`web:` and `conformance:` lines, the failed specs with their first error line and `uptime`. `K7_BED_REF=<a reference e2e-out>` adds a `sameFailures` comparison with that
reference's reports.

| choice | why |
|---|---|
| the Mac only, one bed at a time; a run waits for the bed's slot 1 to be free | k7 is a Mac app for now: Linux and Windows have no bed, and a failure only they show is not looked for |
| the bed is free when its slot-1 lock has no living owner and its ports are silent, and a lock folder in `~/.cache/k7-bed` is held from that decision until the slot is locked | the same slot rule `e2e-macos.sh` applies, read before starting; two agents deciding in the same second would both see it free |
| the bed runs the working tree, and says so when the tree is dirty | the Mac's script runs in the tree it is given |
| one mode, no switch named by `bed.sh` or the bed scripts | every host runs the whole native front; a second mode would test a front no host runs |
| the counters are judged by the bed script itself, 32 s after the suites, while the front is up | each counter line is logged at a 30 s tick: a front stopped before it loses the last line, and a run read short |
| a filtered bed (`E2E_FILTER`) prints the floors it missed and fails on none; a panic fails it all the same | a floor is the whole suite's count: a filtered run touches only what its specs do |
| the engine's poll floor is a quarter of a poll per second of the web suite (`POLLS_PER_SECOND`), one in a conformance-only run; `loads` and `dry` stay counts | the browser polls every two seconds while a project is served, so polls follow the run's wall clock: whole runs read 0.79 to 1.6 a second, and a fixed count fails a fast Mac or a loaded one that polls fine; the fault it catches is a poll that no longer reaches the link, which reads near zero |
| every bed sets `CONFORMANCE_INTERNAL_SECRET` to the front's `K7_FRONT_SECRET` (`k7-dev-front`) | the specs behind the internal seam skip without it, and a skip reads as green |

## The beds are devices

| choice | why |
|---|---|
| the bed starts the front as a device: `env.yaml` carries `STUDIO_LOCAL_USER` (and `K7_INSTANCE: local`), as the tray's `standalone.json` does, and `E2E_USER` is then the local user (`dev-user`) | the server is fading away and its account runtime is gone: the file service, the project pages and `/api/local` exist only on a device, so a server bed would skip or fail the web suite and about 22 conformance specs. A device turns every session into its one local user, so the suite signs in as that user |
| a device has one account and no admin, so the suite signs in as `E2E_USER` alone; what needs a stranger, an author or a refusal is held by the Rust tests, not staged here | `env.yaml` names no `AUTH_ADMIN_GROUPS`, as the tray starts it, so its libraries are the published versions it pins |
| on a device bed every spec holding the account's file service or served project runs in one lane (`playwright.conformance.config.ts`) | the specs' separate accounts are one account here, so two of them at once would cut each other's calls |
| the bench (`/workspaces/bench`, New chain) is not run on a device bed | the bed's `k7-core` is a draft, not a published version a tray installs, and the bench's rig declines (`would-throw`) on it; a server's releases are not made on a device |
| `bed.sh` and `gate.sh` take no mode | there is one bed; a second mode would prove less of what ships |
| what only a server does (the 410 on `/api/files`, the door's refusals, the export routes, the session as a server serves it) is held by the front's Rust tests (`files_hub.rs`, `files_export.rs`, `tokens.rs`, `auth.rs`), not by a spec | a spec cannot meet a server on a device bed, and a Rust test starts either front on its own folder |

## The Mac bed retries once under load

When a Mac bed starts with the 1-minute load average above `K7_BED_RETRY_LOAD` (default: the core count), `e2e-macos.sh` runs Playwright with `--retries=1`; a spec that passes on retry is reported `flaky` and the bed passes, one failing twice fails it.

## Selectors

The app has almost no `data-testid`. Specs select by role and accessible name, which is what a
person reads too: a spec that cannot find a control by its name has found an accessibility gap.
