# Export routes: the folder picker and the file written into it

`/api/local/export/*` is the desktop app's seam to its folder dialog; the page's side is
`apps/k7-daw-frontend/docs/export.md`. The routes are a device's own, like `/api/local` (`local.md`), and sit behind the session.

| choice | why |
|---|---|
| the host installs a `Host` (a dialog and a file manager) with `k7_front::export::set_host`; with none the routes answer 501 | the front is a library in the desktop app and the one process holding both the session and the dialog; the phones' hosts never call it and use their own bridge |
| `POST folder` opens the dialog and returns `{path, name}` or `{cancelled}`; it runs off the async threads | the dialog blocks until the person answers |
| `PUT file?folder=&name=` writes only into a folder the dialog returned during this run | a page script must not turn the route into a write anywhere; the person's pick is the only grant |
| a name is one path segment: no separator, no dot lead, nothing a file system refuses | the name comes from a page; a dot lead is also how a part file is named |
| the body streams to `.name.part`, then is renamed to the first free name (`name (2).ext`) | a half file never bears the name, and the folder holds the person's other files |
| `POST reveal` shows a file of a chosen folder, or the folder, in the file manager | the same grant as the write |
| the last folder is `export-folder.txt` in the front's preferences folder | one device, one person: the next dialog opens there, and the account's own preferences stay free of it |
| the background film (`verb-host.md`) takes `into` from its caller with no check against the picked folders | the verb host is the front's own seam behind its secret, and a command line may name any folder the person's user can write |
