# Repository layout

The `k7-daw` repository (K7 DAW) holds k7 and everything it is built from: the web app, its backend, the
shared TypeScript packages, the C++ engine and its Rust supervisor, the desktop app and the phone shells, and
the module library k7 ships with. It is one npm workspace tree driven from the root
`package.json`.

## Top-level folders

| Folder | What lives there |
|---|---|
| `apps/` | Every program of the repo, one folder each: `k7-daw-frontend` (the web app, with the shared `@studio/*` packages in its `packages/` and the documentation it ships in `user-docs/`), `k7-daw-backend` (the Rust front that is its backend, its crates in `packages/`), `k7-daw-wasm` (the Rust crates built to WebAssembly), `k7-daw-engine` (`k7-runtime`, the C++ audio engine; `k7-supervisor`, the C++ program that runs and watches it; `scripts`, the Node scripts that build and package the native binaries), `k7-daw-android` (the phone app, Gradle), `k7-daw-ios` (the iPhone app, Xcode) and `k7-daw-desktop` (K7 DAW: k7 on a computer, a Tauri app with one window that runs k7's front and shows k7). |
| `k7-core/` | The default module library, as plain files: modules, chains, assemblies, presets, impulse responses, amp models, samples, wavetables. |
| `site/` | The front page of `k7.rocks`, a static HTML file (`scripts/buildSite.mjs` builds it with the documentation). |
| `scripts/` | Root scripts behind the npm scripts: the dev session, the workspace runner, the custom lints, the remote-machine helpers. |
| `demos/` | Small `.k7` demo projects and a `_modules` folder used by them. |
| `.github/` | GitHub Actions workflows and the shared `node` composite action. |
| `.storybook/` | The repository's Storybook configuration. |
| `docs-site/` | This guide and the user guide. |

Root files worth knowing:

| File | Role |
|---|---|
| `package.json` | The workspace list, every root script, the shared dev dependencies, `engines.node >= 22.12.0`. |
| `.nvmrc` | The Node major CI and the bundles share (`24`). |
| `Tiltfile` | The dev session (see [Dev setup](./03-dev-setup.md)). |
| `biome.json` | Lint and format configuration. |
| `knip.json` | Dead-code detection configuration. |
| `vitest.config.ts` | The root Vitest config, used when Vitest is run from the root. |
| `env.example.yaml` | Template for `env.yaml`, the per-machine service configuration (never committed). |
| `.gitattributes` | Forces LF line endings everywhere (`* text=auto eol=lf`) and marks audio, image and binary files as binary. |
| `.npmrc` | `legacy-peer-deps=true`. |

## The workspaces

The root `package.json` declares these workspace globs:

```json
"workspaces": [
  "apps/k7-daw-frontend",
  "apps/k7-daw-frontend/packages/*",
  "apps/k7-daw-desktop",
  "apps/k7-daw-engine/k7-runtime",
  "apps/k7-daw-android/web",
  "e2e"
]
```

| Workspace | Package name | Notes |
|---|---|---|
| `apps/k7-daw-frontend` | `k7-frontend` | React + Vite single-page app. |
| `apps/k7-daw-desktop` | `k7-desktop` | The desktop app; its code is Rust under `src-tauri/`, and the workspace exists for its npm scripts (`stage`, `dev`, `build`). |
| `apps/k7-daw-engine/k7-runtime` | `k7-runtime` | Not JavaScript: the workspace exists for its npm scripts (`dev`, `build:win`, `build:mac`, `build:android`, `test`, `test:all` and the `gen:*` generators). It declares no `build` script, so `npm run build` never compiles C++. |
| `apps/k7-daw-android/web` | `@studio/k7-android-shell` | The Capacitor page of the Android app. |
| `apps/k7-daw-frontend/packages/*` | `@studio/*` | The shared tier, below. |

The supervisor is a crate of the backend, `apps/k7-daw-backend/packages/k7-supervisor`.

## `apps/k7-daw-frontend/packages/`: raw TypeScript through `exports`

Every package under `apps/k7-daw-frontend/packages/` is named `@studio/<folder>` and has no build step. A package's
`package.json` maps public subpaths straight to `.ts` source files:

```json
{
  "name": "@studio/engine-protocol",
  "type": "module",
  "exports": {
    ".": "./types.ts",
    "./project": "./project.ts",
    "./build": "./build.ts",
    "./sessions": "./sessions.ts",
    "./stream": "./stream.ts"
  }
}
```

Consequences of that choice:

- **Only listed subpaths are importable.** `import { x } from "@studio/song/loader"` works because
  `./loader` is in the map; a deep path such as `@studio/song/src/loader.ts` is refused by the
  resolver. Adding a public entry point means adding a line to `exports`.
- **Every consumer compiles the source.** The packages' `tsconfig.json` files use
  `"moduleResolution": "bundler"`, `"allowImportingTsExtensions": true` and `"noEmit": true`, so
  relative imports carry their `.ts` extension. The frontend's Vite build compiles them as part of
  the app; `apps/k7-daw-frontend/vite.config.ts` excludes `@studio/ui-chains`, `@studio/ui-chains-react`,
  `@studio/ui-editor` and `@studio/ui-timeline` from dependency pre-bundling because esbuild's scanner
  rejects the `.ts` extension in their export maps.
- **The backend is Rust.** `apps/k7-daw-backend` is a cargo workspace, no npm workspace; the file service is in the front.
- **`k7.rocks` is a static site** built from `site/` and `docs-site/` (see [Versioning and the static site](./08-versioning-and-site.md)).
- **A package's `build` script is a typecheck.** Most packages declare `"build": "tsc --noEmit"`;
  the exception is `@studio/dsp`, whose `build` runs `tsx src/emit.ts`.

The per-account file service is not a package: it is the Rust crate
`apps/k7-daw-backend/packages/k7-file-service`, held by the front.

Every package is private to the tree.

### The packages by area

| Area | Packages |
|---|---|
| Song files and compile | `song-schema`, `song`, `song-edit`, `k7x`, `song-video`, `song-strudel`, `translator` |
| Commands | `commands` |
| Audio processing | `audio-analysis`, `audio-probe`, `audio-passes`, `take-render`, `dsp` |
| Engine connection | `engine-protocol`, `engine-client`, `engine-browser`, `engine-registry`, `engine-tunnel` |
| Disk | `user-prefs` (the file service and the hub are Rust crates under `apps/k7-daw-backend/packages/`) |
| Media and recording | `ui-mini-player`, `recorder`, `http-client` (the media core is the Rust crate `k7-mediatek`) |
| App shell and views | `project-shell`, `ui-chains`, `ui-chains-react`, `ui-editor`, `ui-eyes`, `ui-timeline`, `ui-waveform`, `ui-module-builder` |
| Shared types | `web-protocol`, `graph-model` |
| Plugin contract | `k7-sdk`, `k7-plugins` |

Each area has its own section in this guide; the package's own `package.json` description and its
`exports` map are the quickest way to see what it offers.

## `apps/k7-daw-engine`, `apps/k7-daw-android` and `apps/k7-daw-ios`

| Folder | Build system | Output |
|---|---|---|
| `apps/k7-daw-engine/k7-runtime` | CMake (`CMakePresets.json`: `macos-clang`, `windows-msvc`, `windows-msvc-ninja`, `dev-gcc`, `release-gcc`, `android-arm64` and others) | The `k7-runtime` executable (or `libk7runtime.so` on Android) |
| `apps/k7-daw-backend/packages/k7-supervisor` | Cargo, in the backend's workspace (`apps/k7-daw-engine/scripts/supervisorBuild.mjs`) | The `k7-supervisor` executable, `tunnel_harness` and, on iOS, the C calls in `k7-embed` |
| `apps/k7-daw-android` | Gradle (`./gradlew assembleDebug`) | The Android APK, which builds its own arm64 engine and supervisor |
| `apps/k7-daw-engine/scripts` | Node | `build-engine.mjs` (build and stage the command-line engine into `dist/k7-runtime/`), `package-release.mjs` (archive it, or rename the desktop app's installers), `devEngine.mjs` and `supervisorBuild.mjs` (used by the dev session) |

The engine section of this guide covers the C++ code itself.

## The UI kit and the build tooling

Seven packages of the frontend's `packages/` are not the song model or the engine: they are what the apps are
assembled with.

| Package | Used for |
|---|---|
| `@studio/ui` | The design-system components and the one icon set (`@studio/ui/icons`). |
| `@studio/i18n` | The Paraglide wrapper the frontend compiles its catalogues with. |
| `@studio/env` | `loadServiceEnv()`, which reads a service's section of `env.yaml`. |
| `@studio/vite-config` | `defineAppConfig()`, the shared Vite config (ports, proxy, PWA, Tailwind). |
| `@studio/tsconfig` | The shared TypeScript bases (`react.json`, `node.json`). |

## `k7-core/`: the default library

`k7-core/` is the module library a new origin, computer or phone starts from. Its folders
are `modules/`, `chains/`, `assemblies/`, `presets/`, `irs/`, `models/`, `samples/` and
`wavetables/`; the binaries in it are ordinary tracked files.

It is consumed in three ways:

- **The server** copies a library listed by an origin but missing on disk from
  `$STUDIO_LIBRARY_SEED` (`apps/k7-daw-backend/packages/k7-libraries`), which can name `k7-core`.
- **The desktop app and phone bundles** carry a copy: `apps/k7-daw-backend/scripts/stageBundle.mjs` writes one
  into the bundle, which the front seeds from (`STUDIO_LIBRARY_SEED`).
- **Lints** read it: `npm run lint:layouts` and `npm run lint:panels` check its module panels.

`biome.json` excludes `k7-core` from linting. The Windows CI jobs set `core.longpaths`, but keep
paths inside it short anyway: a default Windows checkout refuses paths over 260 characters.
