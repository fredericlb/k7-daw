# The last routes

What no family answers ends in `crates/k7-front/src/routes/last/`: the `/api` guard, the built page with its fallback, and the 404 page. The front hands nothing to another process.

## The choices

| choice | why |
|---|---|
| every request no family takes comes here, the one fallback of the router | one tail: half of it elsewhere would answer a path twice or not at all |
| the chain is applied per request, through a router built for that request | what the state keeps never holds the state (`front.md`); the security headers, the loopback lock and CORS are `web.rs`'s |
| the built page is found at `K7_FRONTEND_DIST`, else at the first `apps/k7/frontend/dist` with an `index.html` above the front's folder or binary | the dev session runs the front inside the checkout; a host that stages the front elsewhere names the folder |
| the static answers follow `send` and `express.static`, header for header: the precompressed sibling, mime types and their charset's case, the weak `W/"size-mtime"` tag, `fresh`, a single range span, the folder redirect, the legacy dotfile rule | a browser caches by these headers, and a service worker by the bodies; keeping them keeps every cache a person already holds |
| a refusal of a found file (412, 416) is a JSON body over the file's headers, `Content-Type` included | the conformance spec compares the bytes |
| the 404 page and the URL encoding are written out by hand (`express.rs`) | two short formats; the spec reads them back byte for byte |

## Where the answers differ from `send`'s and Express's, on purpose

| difference | why it is accepted |
|---|---|
| no `ETag` on a JSON answer: the `/api` guard's 401, the door's refusals, the seam's answers | as every JSON answer the front owns (`routes.md`) |
| a refused compressed file's JSON carries no `Content-Encoding` | no client can decode a JSON error labelled `br` or `gzip` |
| the 404 for an upgrade carries the headers hyper writes (`content-length: 0`, `date`) | a client reads the status |
| a method a route does not take carries axum's `Allow` | nothing reads it |
| a body sent to a path nobody answers is not parsed first | a malformed or oversized JSON body no longer wins over the 401 or the 404 |
| on Windows a backslash in a path is a separator for the built page only through `send`'s rules; a request target hyper refuses is a 400 before any route | hyper's 400 is the front's |
