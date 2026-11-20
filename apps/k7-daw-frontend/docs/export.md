# Export: the person chooses the folder

An export (a mixdown, the tracks zip, a video, a cut for another editor) is saved where the person says,
through the system's folder picker. Nothing writes the project's `exports/` any more; the git ignore list keeps it, since old projects hold films there that must never be committed. The Export panel (`components/SharePanel.tsx`) is the surface; `hooks/useShare.ts`
is the one implementation under it and under `project export`; `src/exportHost.ts` is the seam to the host.

## The flow

| choice | why |
|---|---|
| the person picks WHAT first, then **Choose a folder…** runs it | the folder is asked once the format is known, and a palette line (`project export …`) asks the same way |
| the folder is asked BEFORE any work, a render included | a minute of rendering lost to a cancelled dialog is the worst order; a film that runs while the page is closed has nowhere to ask afterwards |
| the result names the folder, with **Show in folder** on a computer and **Share…** on a phone | the person sees where it went; each button exists only where the platform has the thing |
| a clash is numbered ("mix (2).wav"), never overwritten | the folder is the person's and holds their other files |
| a plain browser has no picker: the same panel offers **Download** | the web build and the e2e suite run there; an export is a download, as it always was |
| the last folder is remembered per device and the picker opens there | a device-only app has one person and a habit; it is kept in the device's own settings, never the account's |

## One shape, three hosts

`exportHost()` returns the host the page runs in; the bytes always come from the page or the front, and the host
puts them in the folder.

| host | how the page reaches it | why |
|---|---|---|
| desktop | the front's routes `/api/local/export/*`; the front calls Tauri's dialog plugin through a trait the app installs (`apps/k7-daw-desktop/src-tauri/src/exports.rs`) | the desktop page has no IPC by design (`apps/k7-daw-desktop/docs/desktop.md`), and the front is in the app's process; a plugin already linked (the app's own dialogs use it) costs nothing more |
| iOS and Android | the `k7Ios` / `k7Android` bridge's `call` messages (`pickExportFolder`, `exportBegin`, `exportChunk`, `exportEnd`, `shareExport`) | the phones already speak that protocol for the engine page and the certificate picker; a folder there is a grant only the app holds |
| browser | a download | nothing else exists |

On a phone the bytes cross the bridge in 512 KiB base64 chunks, each awaited, as the phone's own download path
does, so a large video never sits whole in a message. The front is never given a phone's folder:

- **iOS** keeps a security-scoped bookmark (the `id` the page holds); `UIDocumentPickerViewController` picks, `UIActivityViewController` shares.
- **Android** keeps a persisted tree URI; `ACTION_OPEN_DOCUMENT_TREE` picks, `DocumentsContract` creates each file, `ACTION_SEND` shares it.

## The background film

`picture render` renders on the server and outlives the page, so the front writes the film into the folder
itself: the verb takes `into=<folder>` (an absolute path) and the shell fills it from the picker before the
render starts (`ui/commands/project.ts`). The folder is therefore a path the front can write.

| choice | why |
|---|---|
| the desktop and iOS give a path (iOS holds the folder's scope open for the life of the app) | the front writes straight there, part file then rename, so a half film never bears the film's name |
| Android's background film is not offered; the shell says to export the video from the panel | a tree URI is no path, and the front cannot write it; staging the film and moving it needs the page alive at the end, which is what a background render is for not needing |
| without a picker (a browser, the CLI) the verb refuses without `into=` | the film must not land in the project; the sentence says to name a folder |

## The cut

`picture interchange` writes nothing: the cut is its first output line, `useShare` saves that text, and what the
writers left out follows as further lines. The media in an OTIO are named as if the cut sat in an `exports/`
folder beside the project (`apps/k7-daw-frontend/packages/song-video/docs/interchange.md`); a cut saved elsewhere is relinked by the
editor, which every editor offers.

The front's side is `apps/k7-daw-backend/docs/export.md`.
