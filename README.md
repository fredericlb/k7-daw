# K7 DAW (k7-daw)

The music and stage apps of the frediverse.

| app | what it is | dev site |
|---|---|---|
| `apps/k7-daw-frontend` | text-first DAW — the web editor; the origin's front door | `127.0.0.1:5223` |
| `apps/k7-daw-backend` | k7's backend in Rust: the session, the file hub, the engine registry, the built page | `127.0.0.1:8789` |
| `apps/k7-daw-engine/k7-runtime` | the native C++ audio engine (JUCE/Tracktion) | — |
| `apps/k7-daw-engine/k7-supervisor` | the engine machine's supervisor: starts and watches the engine, holds its tunnel | — |
| `apps/k7-daw-desktop` | K7 DAW, the desktop app for Windows, Linux and macOS (Tauri, one window): runs k7's backend, which runs the supervisor and its engine, and shows k7 in its window | — |

`apps/k7-daw-frontend/packages/` is k7-daw's shared tier, all `@studio/*` and consumed as raw
TypeScript source: the song compiler, the command layer, the engine client and
supervisor, the view engines, the waveform component. The engine and its supervisor
serve every audio app, not just k7 — hence their own directory, `apps/k7-daw-engine`. The UI kit (`@studio/ui`),
i18n, http, oidc and the build tooling (`env`, `vite-config`, `tsconfig`) are in that `packages/` too, and the
Rust crates that build to WebAssembly are `apps/k7-daw-wasm`.

## Getting started

```bash
git clone git@github.com:fredericlb/k7-daw.git
cd k7-daw
npm ci
cp env.example.yaml env.yaml   # then fill in the secrets
npm run dev                    # every app, plus the engine
```

In dev the studio is a device on loopback, **http://127.0.0.1:5223** — k7 at the root, no login; in
production k7 runs on the devices (desktop app, phone, Mac). `k7.rocks` is a static site, built from this repository
and served by GitHub Pages: a front page, and the documentation at `k7.rocks/docs` (`docs/site.md`).
Nothing here terminates TLS. See [docs/dev.md](docs/dev.md).

`npm run dev` runs the whole repository as one Tilt session — every app's frontend and
backend, plus a task that builds the C++ engine and runs it,
and rebuilds+restarts it on save. One app on its own is still `npm run dev` inside its
directory. See [docs/dev.md](docs/dev.md).

## What runs on the musician's machine

`apps/k7-daw-engine/k7-runtime` is a CMake/JUCE/Tracktion C++ project, `apps/k7-daw-engine/k7-supervisor` a C++ program
beside it — run from the command line as `k7-supervisor --config engine.json`, or carried by
the desktop app, `apps/k7-daw-desktop`, through k7's backend. The moteur
deliberately declares no `build` script at all, so `npm run build --workspaces --if-present` can
never try to compile C++. They have their own commands:

```bash
npm run build:win --workspace k7-runtime     # or build:mac
npm run engine:build                         # engine + supervisor, staged into dist/k7-runtime/
```

`apps/k7-daw-engine/k7-runtime/build` is large (~30 GB) and fully gitignored. It bakes absolute paths into
its caches, so **moving or renaming the repo requires wiping it** — otherwise the build
fails looking for the old path.

## License

K7 DAW is licensed under the GNU General Public License, version 3 — see [LICENSE](LICENSE).
The notices of the third-party code the engine compiles are in
[apps/k7-daw-engine/k7-runtime/LICENSE](apps/k7-daw-engine/k7-runtime/LICENSE).
