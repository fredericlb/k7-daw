# The libraries in the front

The libraries — `libraries.json`, the library folders, `/account/libraries*` and `/libraries/sync` — are the crate
`crates/k7-libraries`. No relay carries them and nothing is fetched: a device holds the seed its app bundles. The
front answers them itself
(`crates/k7-front/src/routes/files/service/libraries.rs`) once the code root is found (`service.md`).

## The switch

| choice | why |
|---|---|
| the libraries are the front's once a code root is found | the libraries are the whole origin's, so one writer of the folders is the rule |
| made once, at the hub's launch, never reaped | one instance, always kept |
| the store's first `ensure` runs as a task of its own at launch, and the hub's readiness waits for it | a route's answer never waits on a library's first scan, and a standalone's files wait for the seed |

## The store

| choice | why |
|---|---|
| the folder layout, the seeds, the husks and the manifests are fixed: `library.k7x` is written by k7x's own printer | the folders are on every origin; a library rewritten on update would re-sync every device |
| every change takes one lock inside the store, and the copies run on the blocking pool | two copies deciding on the same name would collide; a seed of IR and amp-model binaries never holds a request |
| a copy goes into a dot-named sibling and is renamed into place, eight files in flight | no reader sees half of a version |
| a name from a request is checked as a library name, a version as a folder's spelling, before either becomes a path | a name with `..` or a version named `1e0` never reaches the disk |
| a seed's or a version's links are skipped, never followed | a seed archive with a link must not copy a file from outside it into a public version |
| a dot-named file is neither copied from a draft into a version nor served from one | a draft's `.env` must not become public; the listing already hid dot-names |

## The routes

| choice | why |
|---|---|
| the service answers only a request carrying its secret, compared in constant time; a service made with no secret answers nobody | the gate is the hub's, and a request that is not the hub's is 401 `not from this service's file hub` |
| the admin stamp is the front's: the client's own `x-studio-admin` is never read, only what the front's hub decided is handed in | a forged stamp must not change the list |
| a body is read whole up to 8 MiB (413 `body over 8388608 bytes`), then parsed as `jsonBody` parses it (`{}` when empty or not JSON) | the routes' refusals keep their sentences |
| the hub's readiness (`settled`) is the store's first `ensure` | a standalone's files wait for it |

## Where a device's libraries come from

| choice | why |
|---|---|
| a device holds the bundled seed (`k7-core`, as `libraryArchive.mjs` copies it from the repo) and nothing else of an origin: a library changes with an app update | no server is asked, so a device works with no network and no account |
| a seed that gains a manifest or a higher number installs beside the versions already held, once | an installed version is never rewritten, so a workspace pinned to it keeps its sounds |
| a version a machine lacks is refused by name (`@k7-core 4 is not on this machine`), nothing is fetched | the verbs have no network and the front reads no listing |
| each library's own folder keeps its own state alone; `.sync-*` folders of an earlier install are never read | nothing replicates a library folder any more |

## What the libraries' writes decide

| choice | why |
|---|---|
| the origin has one list of libraries, each a folder with a name, and `@<name>/<type>/<module>` resolves against them from every workspace; no workspace holds a copy | one clone per library, the same for every project |
| an open asks the hub to bring the libraries to their latest and waits only when one is not cloned yet; a sync that fails is logged and the project still opens | every unresolved `@k7-core/...` ref reports itself missing and the graph draws a `?`; the Libraries page shows the failure on the library's row |
| nothing is created before the origin has a library: a workspace and the bench are refused with one sentence, and `Examples` is exempt | a workspace made without a library is a folder whose chains point at nothing, and the refusal belongs before the folder exists |
| a write into a library is refused unless the account is an admin, and an admin writes the draft only, the published version being refused to everybody with a message saying to edit the draft | a library is every account's, so a write is everybody's at once; the rule covers the file tree, the builder's Save and a chain edited from the Libraries tab alike |
| the admin stamp decides on every request, not at an open | a first write before any open must not be refused to an admin, and an account whose groups changed must not need a restart |
| a request of another role on the same account that lands inside an admin call's awaits is a known limit | overlays per request would close it and are the native side's, which resolves a library's folder per request |
| a write reaches the draft only, and a workspace sees it once it pins `draft` or the draft is published and the workspace pins that | a published version never changes |
| a library's own folder and `_libraries` cannot be renamed, moved or deleted from a workspace; below `_libraries/` only a library's name or `account` resolves | which libraries the origin has is its admins' business, and a path there would let any account write a library |
| a deletion inside a library moves into that library's own `_recycle_bin/` | a library may sit on another disk than the workspace and a move cannot cross disks |
| the account's own library is its account's to write and never refused, local to each device and kept out of every list of workspaces | it travels nowhere: it is no room and no origin's library |
| frozen chains are written into the account's own library as ordinary chains | every chain reader already walks the account's library first |
| the machine's standing answers (which chain separates, which makes an instrumental) are its settings, answered before a workspace is resolved, chosen among the libraries' cold chains | no project is open when they are set |
| a model a chain needs is fetched from the module's own `model_url`, never one the caller sends, and a model already present answers `job: null` | a route can only download what a library already names; a stale menu costs nothing |
| a chain's bench edits no origin library and changes no overlay, and what `chain save` writes from it goes to the account's own library | a bench is generated and keeps nothing |

## Where it answers differently, on purpose

| difference | why |
|---|---|
| a library version of a request that is not a whole number from 1 to 999999 is a `failed` note, never a path | `String(1.5)` named a folder |
| a legacy `library.yaml` is read as flat `key: value` lines; anything nested or folded reads as no manifest | no YAML library in Rust (decision 4-3 is open); the hub itself only wrote flat ones |
| a body that is JSON but not an object reads as `{}` | a field of `null` must not be a 500 |
| an error note carries the operating system's wording of Rust, not `ENOENT: …` | the state of the note is what callers read |

## The gates

| gate | result |
|---|---|
| `crates/k7-libraries/tests/vectors/libraries.json` replayed (`tests/all/goldens.rs`): the store's cases and the routes; each answer and the tree left behind compared whole | frozen: the goldens are edited as data, never regenerated |
| `tests/all/hostile.rs`: names with `..`, a forged or missing stamp or secret, a seed with a link, a dot-file | each case seen red under its mutation |
| `tests/all/seeds.rs`: the seed is added once, the husks are swept and a version holding a file is kept | over fixture folders |
| `crates/k7-front/src/routes/files/service/libraries.rs`: a client cannot stamp itself an admin, every refusal keeps its sentence | unit tests |
