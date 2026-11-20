# Shared UI package: the mini player

One small package exists because more than one screen of the studio draws the same thing. It is raw TypeScript consumed as source, take React as a peer, and take every string as a prop.

## `@studio/ui-mini-player`

`apps/k7-daw-frontend/packages/ui-mini-player/src/MiniPlayer.tsx` is the transport bar at the bottom of a screen: an icon and title (with free-form `meta` and `status`), play/pause, a scrubber with `clock()` times, a close button, and optional children. It **owns no transport**: position, duration and playing state are props and every button is a callback, because what it plays is the host's business — k7's reference tracks (`apps/k7-daw-frontend/src/components/ReferenceBar.tsx`) are the host. The scrubber is disabled until a duration is known. The bar is **one height everywhere** (40 px with a mouse, 60 px on a device without hover), so no page moves under it when a track starts, and a slow wave drifts behind it (`apps/k7-daw-frontend/packages/ui-mini-player/src/Wave.tsx`) whose speed follows an optional `level()` getter — a function rather than a number, so a meter at 20 Hz re-renders nothing.

`apps/k7-daw-frontend/packages/ui-mini-player/src/mediaSession.ts` (`useMediaSession`) mirrors the same props into the browser's Media Session API (title, artist, album, position; play, pause, stop, seek, skip forward/back and, when `onNext` is given, next track), so a phone's lock screen and a headset's buttons drive the same callbacks. On the Android app the WebView has no Media Session API, so the app stands one in and relays these calls to a native session.

All labels (`region`, `play`, `pause`, `position`, `close`) come from the host's catalogue through `MiniPlayerLabels`.

## Checks

Neither package has tests; `npm run typecheck` in each compiles it. Their behaviour is checked through their hosts in the running app.
