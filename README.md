# K7 DAW (k7-daw)

The music and stage apps of the frediverse.

| app | what it is | dev site |
|---|---|---|
| `apps/k7` | text-first DAW — the web editor; the origin's front door | `127.0.0.1:5223` |
| `engine/k7-runtime` | the native C++ audio engine (JUCE/Tracktion) | — |
| `engine/k7-supervisor` | the engine machine's supervisor: starts and watches the engine, holds its tunnel | — |
| `apps/k7-tray` | k7 on Windows and Linux (Tauri, in the system tray; a Mac runs the iOS app): runs k7's backend, which runs the supervisor and its engine; k7 opens in the browser | — |

`packages/` is k7-daw's shared tier, all `@studio/*` and consumed as raw
TypeScript source: the song compiler, the command layer, the engine client and
supervisor, the view engines, the waveform component. The engine and its supervisor
serve every audio app, not just k7 — hence their own directories. The UI kit (`@studio/ui`),
i18n, http, oidc and the build tooling (`env`, `vite-config`, `tsconfig`) are in `packages/` too.

## Getting started

```bash
git clone git@github.com:fredericlb/k7-daw.git
cd k7-daw
npm ci
cp env.example.yaml env.yaml   # then fill in the secrets
npm run dev                    # every app, plus the engine
```

In dev the studio is a device on loopback, **http://127.0.0.1:5223** — k7 at the root, no login — and its
documentation answers at **https://docs.priv.k7.rocks**; in production k7 runs on the devices
(tray, phone, Mac), and the server only serves `k7.rocks` and `docs.k7.rocks`
(`docs/images.md`).
Nothing here terminates TLS. See [docs/dev.md](docs/dev.md).

`npm run dev` runs the whole repository as one Tilt session — every app's frontend and
backend, plus a task that builds the C++ engine and runs it,
and rebuilds+restarts it on save. One app on its own is still `npm run dev` inside its
directory. See [docs/dev.md](docs/dev.md).

## What runs on the musician's machine

`engine/k7-runtime` is a CMake/JUCE/Tracktion C++ project, `engine/k7-supervisor` a C++ program
beside it — run from the command line as `k7-supervisor --config engine.json`, or carried by
the tray, `apps/k7-tray`, through k7's backend. The tray is in no image —
each `docker/Dockerfile` names the workspaces it installs rather than globbing. The moteur
deliberately declares no `build` script at all, so `npm run build --workspaces --if-present` can
never try to compile C++. They have their own commands:

```bash
npm run build:win --workspace k7-runtime     # or build:mac
npm run engine:build                         # engine + supervisor, staged into dist/k7-runtime/
```

`engine/k7-runtime/build` is large (~30 GB) and fully gitignored. It bakes absolute paths into
its caches, so **moving or renaming the repo requires wiping it** — otherwise the build
fails looking for the old path.

## Images

See [docs/images.md](docs/images.md) — the images CI builds and what each holds.

## License

K7 DAW is licensed under the GNU General Public License, version 3 — see [LICENSE](LICENSE).
The notices of the third-party code the engine compiles are in
[engine/k7-runtime/LICENSE](engine/k7-runtime/LICENSE).
