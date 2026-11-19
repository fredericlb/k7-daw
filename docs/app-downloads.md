# App downloads and updates

Builds of the APK, the command-line engine and the tray are published on k7 itself, at
**`https://k7.rocks/app_downloads`**: one page anyone can open, no login, and the feeds the tray and
the Android app update themselves from. No CI job builds or publishes them: a developer builds an
artefact locally and sends it to the server the way the Publishing section describes.

## Two stores

The apps' builds sit at the root (`app_downloads/<channel>/…`, `app_downloads/<channel>.json`),
the tray's under `tray/` (`app_downloads/tray/<channel>/…`). The page lists both.

| choice | why |
|---|---|
| a store per product, not the tray's files in the apps' | a publish replaces its stamp's whole folder, and the two publish the same stamp apart: sharing one, each would drop the other's files |
| the APK and the engine archives at the root | installed Android apps read `<channel>.json` there |

## Two channels

A build is named by its release stamp ([versioning.md](versioning.md)), and the stamp says its
channel: `vX.Y.Z` is **`release`**, `snapshot-<commit>` is **`snapshot`**. The page shows both, the
release first. An installed app follows its own channel and never crosses to the other.

| choice | why |
|---|---|
| a channel per stamp kind, not one feed | a snapshot's version (`0.1.0-snapshot-9f2c1ab`) is a prerelease of the release it was cut from: on one feed, a release would pull every snapshot install onto it and leave it there |
| on the snapshot channel, any other build is newer | snapshot versions sort by commit hash, not by date; the channel only ever holds `main`'s latest build, so "another commit" is "later" |
| the rules written once, in `engine/scripts/releaseStamp.mjs` | the shell steps call it; the tray (Rust) and the phone (Gradle, Kotlin) build without Node, so their copies stay and point at it, and its test pins what all of them answer |
| `current` and `previous` kept per channel, nothing else | an app that read its feed just before a publish still finds its file; older builds are on GitHub |
| on the root domain, `/app_downloads` is the one address besides the coming-soon page | the image's other host, `app.k7.rocks`, answers the same path and is where CI publishes; no host serves the app itself, which runs on devices, and `k7.rocks` is a front page ([apps/k7/backend/docs/site.md](../apps/k7/backend/docs/site.md)) |

## Publishing

A developer sends every file of a build's folder to
`https://app.k7.rocks/api/app-releases/<stamp>/<file>` (one `PUT` each, raw body) and then `POST`s
`/api/app-releases/<stamp>/publish`, which switches the channel to that build at once. The tray's go
under `/api/app-releases/tray/`, with the updater's `latest.json` among its files.

| choice | why |
|---|---|
| a `400`, `401` or `413` is never retried | the server meant it; retrying a bad token or an oversized file only delays the red |
| a retried `PUT` sends the whole file again | the server writes it beside its name and renames it, so a cut upload leaves nothing behind |

Both need `Authorization: Bearer <APP_RELEASE_TOKEN>`, a value only the uploader and the server's
`APP_RELEASE_TOKEN` (`docker/.env`) know. Unset on the server, every upload answers 401. To set it
up, generate one value (`openssl rand -hex 32`) and put it in both.

A file is at most `APP_RELEASE_MAX_MB` (2048); its name is letters, digits, `.`, `_` and `-`, which
is why `engine/scripts/package-release.mjs` names every release file for its stamp — `latest.json`
must name the very file the server holds. The builds live under `STUDIO_APP_RELEASES_DIR`,
`/data/app-releases` in the image (the `data` volume), one folder per channel, and the tray's
channels in `tray/` beside them. The routes are the Rust front's (`apps/k7/backend-rs/docs/routes.md`), on the server's image only:
the front of the tray or a phone does not answer them.

## K7 DAW

K7 DAW (the desktop app, `apps/k7-tray`) reads `app_downloads/tray/<channel>/latest.json` with tauri-plugin-updater
([apps/k7-tray/docs/tray.md](../apps/k7-tray/docs/tray.md)).

| choice | why |
|---|---|
| every update signed, the private key held by whoever builds the tray (`TAURI_SIGNING_PRIVATE_KEY`) | the upload token alone would let whoever holds it ship code every tray runs; the public key is built into the tray from `K7_UPDATER_PUBKEY` |
| a build without the public key has no updater | rather than one that trusts anything |
| the channel from the release stamp, `dev` asks nothing | a developer's build is not on any feed |
| Windows, the AppImage and the macOS app update themselves; a `.deb` is told | the updater installs those; a `.deb` belongs to apt, so Download opens the page |
| the feed keeps its `tray/` path; files are `k7-daw_<stamp>_<os>-<arch>.<ext>`, the macOS updater's archive an `.app.tar.gz` beside the `.dmg` | installed apps poll that URL for good, and `latest.json` carries the file URLs |

## The Android app

The Android app (`engine/k7-android/app/src/main/kotlin/se/frediver/k7/Updates.kt`) reads the root
store's `<channel>.json`: it downloads a newer APK on an unmetered network and offers it, and the
person installs it — Android always asks the first time, and lets later updates through once k7
installed itself (Android 12+). The token is the only guard on what reaches that feed; the signature
check is the second: an APK not signed with the release key never installs
([engine/k7-runtime/docs/android.md](../engine/k7-runtime/docs/android.md)). The iPhone app ships
through Xcode.
