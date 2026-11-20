# Downloads under way

`src/downloads.ts` is the page's one list of what is coming down: the copies a page starts, a model the file service fetches, an instrument the supervisor fetches.
The sidebar's download manager (`apps/k7-daw-frontend/src/components/DownloadsButton.tsx`) draws it, and a
row that started a download reads its spinner from it.

## Choices

- **A module-level store, not React state.** A screen and k7's sidebar are two packages that
  cannot import each other; this package is the one both already depend on. It stays
  framework-free: each screen reads it with `useSyncExternalStore`.
- **A key names the thing, not the click.** `model:<job>`, `instrument:<name>`. Queuing a key already
  queued or running answers the same promise, so a second click fetches nothing twice.
- **The copies are queued one entry per file, two at a time.** The backend answers a copy only once it
  is whole and reports no progress, so a batch POST would be one spinner for the lot and a count that
  jumps from 0 to N. One entry per file is what makes "done / total" mean something, and two at a time
  keeps the server and the disk from being asked for everything at once while a second file still
  overlaps the first one's round trips.
- **What others run is mirrored, not queued.** A model job runs on the file service and is polled
  (`trackDownload`); the supervisor keeps its own instrument queue and pushes it whole (`syncDownloads`,
  where a row that leaves the list while running has landed).
- **A finished mirrored row is handed over once.** A backend keeps an ended job listed a few seconds so
  a poll sees it end; the mirror then stops passing it, or the next poll would draw a row the manager
  had already let go of after its linger.
- **Finished rows linger four seconds once nothing runs**, then go; a failure stays until Clear, since
  it is the one row that asks for something.
