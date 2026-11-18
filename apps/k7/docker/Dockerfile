# syntax=docker/dockerfile:1
#
# k7 — the web editor, ONE process: the backend serves its own built
# frontend, no nginx, and this image starts from a plain Node image.
#
# The first three stages see only the manifests, so the installs are cached across every push that
# leaves them alone — docs/images.md. Node is 24, the one version production and CI share (same page).
#
# Build from the repo root: docker build -f apps/k7/docker/Dockerfile .

# ---- manifests: the lockfile and every package.json, nothing else -----------
FROM node:24-alpine AS manifests
WORKDIR /src
COPY . .
RUN mkdir /out && { find . -name package.json -not -path '*/node_modules/*'; echo ./package-lock.json; echo ./.npmrc; } | tar -cf - -T - | tar -xf - -C /out

# ---- toolchain: ssh2 (k7's media sftp) builds its optional crypto binding
FROM node:24-alpine AS toolchain
RUN apk add --no-cache python3 make g++
WORKDIR /app
COPY --from=manifests /out ./

# ---- deps: the whole workspace, which every frontend build starts from ------
FROM toolchain AS deps
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund

# ---- frontend build ----------------------------------------------------------
FROM deps AS frontend-build
COPY . .
# The build stamp shown in the SPA's chrome — docs/versioning.md.
ARG APP_VERSION=dev
ENV VITE_APP_VERSION=$APP_VERSION
# vite alone: the lint job typechecks every workspace (docs/images.md).
RUN npm exec --workspace k7-frontend -- vite build
# The backend and the file service as plain JS, reading the checkout around them: no tsx at run time (docs/images.md).
RUN node apps/k7/backend/scripts/bundle.mjs server-dist --in-tree
# The page `picture render` runs in chromium, built here since nothing builds it at run time (backend-rs/docs/service.md).
RUN node apps/k7/backend-rs/crates/k7-picture/page.mjs /app/render-page/renderPage.js
# What the Rust front's actor reads at its code root, a folder of its own (apps/k7/backend-rs/docs/service.md).
RUN node apps/k7/backend-rs/scripts/stageCodeRoot.mjs /code-root

# ---- backend prod deps: k7-backend's closure only, no devDependencies -------
FROM toolchain AS backend-deps
RUN --mount=type=cache,target=/root/.npm npm ci --workspace k7-backend --omit=dev --no-audit --no-fund

# ---- k7's Rust front: a static musl binary, from apps/k7/backend-rs and the waveform module it embeds ----------
FROM rust:1.97.1-alpine AS front-build
WORKDIR /src/apps/k7/backend-rs
COPY apps/k7/backend-rs ./
# The stream copies' encoder, TypeScript's own module, decoded into the binary at build (apps/k7/backend-rs/docs/mediatek.md).
COPY packages/audio-analysis/src/waveformWasm.generated.ts /src/packages/audio-analysis/src/
# The waveform crate the k7x reader links by path, and the version build.rs reads.
COPY packages/audio-analysis/rust /src/packages/audio-analysis/rust
COPY apps/k7/package.json /src/apps/k7/package.json
# The k7x reader the compile host links by path (apps/k7/backend-rs/docs/compiler.md).
COPY packages/k7x/rust /src/packages/k7x/rust
# The song compiler the compile host runs, its schema, and the checked-in module whose `SOURCES` it reports (packages/song/docs/rustCompilerHost.md).
COPY packages/song/rust /src/packages/song/rust
COPY packages/song-schema/rust /src/packages/song-schema/rust
# The song editor the file service's native writes stand on (apps/k7/backend-rs/docs/file-edits.md).
COPY packages/song-edit/rust /src/packages/song-edit/rust
COPY packages/song-schema/src/builtinParams.generated.json /src/packages/song-schema/src/
COPY packages/k7-wasm/src/k7Wasm.generated.ts /src/packages/k7-wasm/src/
# The root domain's drawing, which the site page inlines (apps/k7/backend-rs/docs/last.md).
COPY apps/k7/frontend/icon/k7-splash.inline.json /src/apps/k7/frontend/icon/
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/src/apps/k7/backend-rs/target,sharing=locked \
    cargo build --release --locked -p k7-front \
 && cp target/release/k7-front /k7-front

# ---- final image --------------------------------------------------------------
FROM node:24-alpine AS runtime
# ffmpeg for video and an mp3 mixdown — audio is decoded and encoded in WebAssembly.
RUN apk add --no-cache ffmpeg
# tini as PID 1: it reaps whatever a dead file service left re-parented to it, and forwards the stop (docs/images.md).
RUN apk add --no-cache tini
# chromium and its fonts: `picture render`, the picture drawn headless (packages/file-service/docs/serverRender.md).
# Alpine's chromium has no SwiftShader: Mesa's lavapipe is the Vulkan its WebGL2 draws on (crates/k7-picture).
RUN apk add --no-cache chromium font-noto ttf-freefont mesa-vulkan-swrast vulkan-loader
ENV STUDIO_CHROMIUM=/usr/bin/chromium-browser
# Before the VOLUME lines, which freeze them: a fresh volume copies this owner (docs/images.md).
RUN mkdir -p /data /library /workspaces \
 && chown node:node /data /library /workspaces \
 && chmod 700 /data /library /workspaces
COPY --from=front-build /k7-front /usr/local/bin/k7-front
COPY --chmod=755 apps/k7/docker/start.sh /usr/local/bin/k7-start
COPY --from=frontend-build /code-root /usr/local/share/k7/code
ENV K7_CODE_ROOT=/usr/local/share/k7/code
# Express hides its stack traces.
ENV NODE_ENV=production
WORKDIR /app
COPY --from=backend-deps /app ./
# The whole repo: @studio/* are symlinks to raw TypeScript source, so a missing package is a crash on import.
COPY . .
# The built SPA — apps/k7/backend/src/server.ts serves this directly.
COPY --from=frontend-build /app/apps/k7/frontend/dist ./apps/k7/frontend/dist
COPY --from=frontend-build /app/server-dist ./server-dist
COPY --from=frontend-build /app/render-page ./render-page
ENV K7_RENDER_PAGE=/app/render-page/renderPage.js
# node's compile cache, in `node`'s own home: the backend skips compiling its bundle again (docs/images.md).
ENV NODE_COMPILE_CACHE=/home/node/.cache/node-compile-cache
# The front's compiled modules, in `node`'s home, never a volume (docs/images.md).
ENV K7_FRONT_CACHE=/home/node/.cache/k7-front

# k7's per-person preferences (packages/user-prefs/docs/store.md).
ENV STUDIO_PREFS_DIR=/data/prefs
# The apps' builds served at /app_downloads, the current and previous one per channel (docs/app-downloads.md).
ENV STUDIO_APP_RELEASES_DIR=/data/app-releases
VOLUME ["/data"]
# The instance's data folder (`.auth`, `.shares`, `.engines`) IS this volume.
ENV STUDIO_MEDIATEK=/library
VOLUME ["/library"]
# The accounts' workspaces, one folder each, when the deployment sets $STUDIO_FILES_ROOT to it.
VOLUME ["/workspaces"]

# The front's port; node listens behind it on 127.0.0.1:8790 (start.sh), and Traefik is the one hop before it.
EXPOSE 8789
ENV K7_FRONT_TRUST_HOPS=1
# The stamp /api/health reports beside the version, last so a new one rebuilds no layer above (docs/versioning.md).
ARG APP_VERSION=dev
ENV APP_VERSION=$APP_VERSION
WORKDIR /app/apps/k7/backend
# Not root: the code under /app is root's and read-only to it; only the volumes above are its own.
USER node
# Readiness, not liveness: the volumes take a write (docs/images.md).
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
  CMD wget -qO- http://127.0.0.1:8789/api/ready || exit 1
# The bundle under plain node, behind the front; start.sh closes the volumes again and runs both (docs/images.md).
ENTRYPOINT ["/sbin/tini", "--"]
CMD ["/usr/local/bin/k7-start"]
