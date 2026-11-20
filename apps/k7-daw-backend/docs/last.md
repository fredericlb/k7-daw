# The last routes

What no family answers ends in `apps/k7-daw-backend/packages/k7-front/src/routes/last/`: the `/api` guard, the built page with its fallback, and the 404 page. The front hands nothing to another process.

## The choices

| choice | why |
|---|---|
| every request no family takes comes here, the one fallback of the router | one tail: half of it elsewhere would answer a path twice or not at all |
| the chain is applied per request, through a router built for that request | what the state keeps never holds the state (`front.md`); the security headers, the loopback lock and CORS are `web.rs`'s |
| the built page is found at `K7_FRONTEND_DIST`, else at the first `apps/k7-daw-frontend/dist` with an `index.html` above the front's folder or binary | the dev session runs the front inside the checkout; a host that stages the front elsewhere names the folder |
| the page is served by tower-http's `ServeDir`: the `.br` or `.gz` the build wrote beside a file, ranges, `Last-Modified` and conditional GETs, the folder redirect | a maintained library does what a hand-written copy of Express's static serving did; the answers' bytes were never a contract |
| a page address no file answers gets `index.html`; `/assets/`, `/api` and `/auth` never do | a page would parse a missing asset as a script, and an API client would read a shell as its answer |
| a file whose name starts with a dot is never served | the build holds none; the guard keeps it so |
| the 404 is a short text, `not found` | no client reads it |

## Where the answers differ, on purpose

| difference | why it is accepted |
|---|---|
| no `ETag` on a file: `Last-Modified` and `If-Modified-Since` answer a conditional GET | a hashed asset is immutable and the shell is asked again; no cache depends on the tag |
| no `ETag` on a JSON answer: the `/api` guard's 401, the door's refusals | as every JSON answer the front owns (`routes.md`) |
| the 404 for an upgrade carries the headers hyper writes (`content-length: 0`, `date`) | a client reads the status |
| a method a route does not take carries axum's `Allow` | nothing reads it |
| a body sent to a path nobody answers is not parsed first | a malformed or oversized JSON body no longer wins over the 401 or the 404 |
