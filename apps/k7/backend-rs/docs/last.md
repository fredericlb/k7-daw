# The backend's last routes

`K7_LAST_ROUTES=front` hands the front what the TypeScript backend still answers once the families
are the front's: the seam's TypeScript half, the root domain's page,
the `/api` guard, the built page with its fallback, and
Express's own 404. With it on, the front asks node nothing, so a host can start the front alone
(`docs/rust-phase6-plan.md`, 6.5). The code is `crates/k7-front/src/routes/last/`.

## The choices

| choice | why |
|---|---|
| one switch for all of them, off by default | they are one tail, `server.ts` after its families; half of it in each process would answer a path twice or not at all |
| it holds only with `K7_FILES=front` and the file hub started, else the front says so and keeps proxying | without the hub here, node runs the hub, and its paths would reach a front that answers none of them |
| the last routes stand where the proxy stood: every request the front declines comes here instead of going to node | the front already sends to node exactly what node answers; replacing that one call changes no family's routing |
| the chain is applied per request, through a router built for that request | what the state keeps never holds the state (`front.md`); the security headers, the loopback lock and CORS are `web.rs`'s, unchanged |
| a bearer is verified here: the hub's own ephemeral bearers in memory, then `library-tokens.json`, its use noted and written within a minute | node held the ephemeral bearers it minted; with the hub in the front they are the front's, and the listed ones are the file both processes already share |
| a verified bearer signs nobody in: the preferences still read the session, and `/api/ai/*` and `/api/token*` are the 404 | `bearerOrSession` sets a bearer's user, then the preferences' own `requireAuth` reads the session again; the backend keeps no copy of the others behind its guard |
| `verify-token` on the seam answers for the backend | the engine slots keep asking their seam, which is this front once it holds the hub; no caller changes |
| the hub takes its family without asking node whether it runs one | node is not asked anything under the switch; the hub's own seam already answers `/hub` for itself |
| the built page is found at `K7_FRONTEND_DIST`, else at the first `apps/k7/frontend/dist` with an `index.html` above the front's folder or binary | the dev session and the image run the front inside the checkout, where node found its own `../../frontend/dist`; a host that stages the front elsewhere names the folder |
| the static answers are `send` 0.19's and `express.static`'s, header for header: the precompressed sibling, mime 1's types and their charset's case, the weak `W/"size-mtime"` tag, `fresh`, range-parser's single span, serve-static's folder redirect, the legacy dotfile rule | a browser caches by these headers, and a service worker by the bodies; byte-compatible answers mean a host switching keeps every cache it had |
| a refusal of a found file (412, 416) is the backend's JSON over the file's headers, `Content-Type` included | serve-static forwards a found file's errors to the error handler, which keeps what `send` had set; the conformance spec compares the bytes |
| the site page is built from the frontend's `icon/k7-splash.inline.json` at compile time, sent with Express's body ETag and its 304 | the same drawing as the TypeScript imports; the image's Rust stage copies that one file |
| Express's 404 page, `res.redirect`'s negotiated body and `encodeurl` are written out by hand (`express.rs`) | three short formats; the spec reads them back byte for byte |

## Where the front answers differently, on purpose

| difference | why it is accepted |
|---|---|
| no `ETag` on a JSON answer: the `/api` guard's 401, the door's refusals, the seam's answers | as every JSON answer the front owns (`routes.md`) |
| a refused compressed file's JSON carries no `Content-Encoding` | node labels its JSON error `br` or `gzip`, which no client can decode |
| the 404 for an upgrade carries the headers hyper writes (`content-length: 0`, `date`) | node writes the status line and `Connection: close` alone; a client reads the status |
| a method a route does not take carries axum's `Allow` | it was already so through the proxy; nothing reads it |
| a body sent to a path nobody answers is not parsed first | node's shared parser refused a malformed or oversized JSON body before the 401 or the 404 |
| on Windows a backslash in a path is a separator for the built page only through `send`'s rules; a request target hyper refuses is a 400 before any route | node on Windows read the same; hyper's 400 was already the front's |
