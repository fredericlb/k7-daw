# The account's media in native Rust

A clip's waveform and its prewarm, the prefetch of a playlist's records onto the bound
machine, a take kept in the project's record directory and a module's model downloaded, held by the front (the TypeScript's own
`waveformOps.ts`, `waveformCache.ts`, `recorderSave.ts` and `modelDownloads.ts` are the referee). This page holds the choices and what forced them; the numbered rows (2400
to 2499) are added as they are built.

## What the media cost

Measured on the Mac (Apple M5 Pro, load average 4 to 18 from other agents), node 26 with the TypeScript's own
modules (`--import tsx`), and the front's crates in a scratch test, a scratch home, 3 runs each:

| measure | node (the TypeScript) | Rust, crates at opt-level 0 | Rust, `k7-mediatek` and `k7-js` at opt-level 3 |
|---|---|---|---|
| a sidecar made from 10 min of 48 kHz stereo WAV (115 MB), the module decoding | 230 ms | 350 ms | 350 ms |
| the same from 5 min of 192 kb/s MP3, the module decoding | 310 ms | 430 ms | 425 ms |
| the WAV, the engine's `--decode` | 244 ms | 2,080 ms | 234 ms |
| the MP3, the engine's `--decode` | 360 ms | 1,076 ms | 367 ms |
| the freshness check of 750 / 3,000 sources, cold (a 290 kB sidecar parsed, 128 kB fingerprinted each) | 537 / 1,991 ms | 4,158 / 16,787 ms | 439 / 1,765 ms |
| the same, warm (two `stat`s each against the remembered stamps) | 3.2 / 11.7 ms | — | — |
| `sidecar::present` over 750 / 3,000 (no parse, no fingerprint) | — | 13 / 52 ms | 13 / 53 ms |
| a 256 MiB model through a server on `127.0.0.1` | 241 to 258 ms, about 1,000 MiB/s | 134 to 201 ms with `sync_all`, 1,270 to 1,910 MiB/s | — |
| a region sliced and its grid, 1,000 / 4,000 buckets, no warp | 0.17 / 0.21 ms | — | — |
| the same with a warp map of 200 markers | 0.75 / 2.0 ms | — | — |

| what was read | what it decides |
|---|---|
| every sidecar the Rust wrote was byte-identical to node's, for both files and both decoders | the analysis stays `k7-mediatek`'s (the very WebAssembly module in Wasmtime, the engine's `--decode`); nothing is ported again |
| the MP3's sidecar from the engine and from the module differ from byte 406, in node as in Rust | the native side picks the decoder as the TypeScript does, the engine binary first; a different choice is a different sidecar |
| at opt-level 0 the engine path and the freshness check are 9 times slower: their per-sample and per-byte loops are `k7-mediatek`'s own code, which the dev profile leaves unoptimized | the debug front (Tilt's session, the bed) draws 9 times slower than a release; a test's bound is a count |
| a decode is a third of a second for ten minutes; the cold freshness check is 0.6 ms a source and linear (n against 4n: 3.7 to 4.0 times) | the gain is the queue and the state's life, not speed; a project of 3,000 clips costs 1.8 s cold on the blocking pool, then 12 ms |
| the warp's lookups are linear in the markers, once per bucket and per onset | buckets times markers: a hand-written map of 10,000 markers would cost about 100 ms a waveform; the native lookup bisects a monotonic map |
| loopback moves a model at a gigabyte a second either way | the download's limits are the network's; its risks are a stall, a short body and a compressed one |

## Where the line runs

| choice | why |
|---|---|
| the front owns the envelopes' memo, the decodes in flight, the slicing, the prewarm, the model registry, the take's checks and the prefetch's timer | each waits on a decode, a socket or a timer, and the page polls the prewarm every 350 ms; none waits behind a verb |
| the sidecar's arithmetic and text stay `k7-mediatek`'s | one producer of a sidecar for the file service and this piece; node's own vectors already hold it (`apps/k7-daw-backend/packages/k7-mediatek/tests/analysis.rs`) |
| the `waveform` crate is not linked natively | Wasmtime's 350 ms for ten minutes is not what a person waits on, and a second build of the measure is a second producer to keep byte for byte |
| `/account/cold-chains/download` is native where chains and media both answer (1p-D16, `file-chains.md`) | the route finds the chain, then starts each model in 1l's registry (the same one `/models/download` feeds); `/models/download` is native too |

## On disk and on the wire

A sidecar keeps its name, `<source>.cache`, and its bytes. It is written under a temporary name that also ends in
`.cache`, so the tree, the capture and the revision walk skip it as they skip a sidecar, then renamed over: a reader
never parses half a sidecar, and a link planted at the sidecar's name is replaced, never written through. A model
is written under `<file>.part-<uuid>`, synced, then renamed; the loader only ever opens the final name. The jobs
path's `prefetch` request is unchanged.

## The crate's model (`k7-file-service/src/media/`)

No I/O and no clock of its own: the front (L2) feeds it sidecars, a clock and random bytes.

| choice | why |
|---|---|
| `Region` keeps the display mapping in private fields and offers `to_display` as a method; a marker copy lives there, not in a public field | the answer's shade and every onset go through the one mapping, and a public `markers` field or a lifetime on `Region` would change the fixed shape |
| a warp lookup bisects an axis checked once to be non-decreasing and NaN-free, else walks as the TypeScript does (`warp.rs`, 2421) | the walk takes the first segment whose far end is at or past `x`; a bisect lands there only on an ordered axis, and a NaN probe falls on the last segment; a test counts the markers read at n against 4n (`take_probes`), since a walk passes any time bound |
| `Math.max`, `Math.min`, `Math.round` and `\| 0` are written out, not replaced by their nearest Rust method | NaN propagates through `Math.max`, `Math.round` rounds a half up, and `\| 0` wraps at 2^32; the vectors hold each |
| `resolve_onsets` asks a sorted set for the nearest neighbour on each side (2422) | `\|a - b\|` computed in f64 only grows away from `a`, so two neighbours decide it, in n log n; a test counts the `near` calls at n against 4n (`take_nears`) |
| the models registry (`Models`) is told what happened and answers what to fetch; ids come from random bytes it is given, the clock is the caller's | a test turns the clock past the ten minutes a finished job is kept, and the machine needs no runtime |
| the route's clamps are `clamp_buckets`, `clamp_sensitivity` and `is_whole`, on the query value as text | `Number(text)` of the query is the TypeScript's reading, and the cut to a whole number (2427) is one place |

## The front's I/O (`k7-front/src/routes/files/service/native/media/`)

An account's `Media` holds the envelopes' memo, the sliced memo, the prewarms' statuses, the downloads' registry and its counts, and lives in the account's `Held`; the take's push is a function.

| choice | why |
|---|---|
| the envelope memo is per account, keyed by the source's real path, gated by mtime, held to 16 MiB by least recently used and let go after ten idle minutes (2410) | the TypeScript's map is bounded by a runtime's heap; the front's is every account's |
| the freshness memo remembers a YES against both files' size and mtime and never a NO, and is emptied when it reaches 20,000 entries (2411) | two `stat`s cost less than a parse and a fingerprint; a count that is never reached in practice needs no ordering |
| one decode per (real path, threshold) in the process: a request and the prewarm join it, only requests are counted, and a prewarm's cancel stops the decodes no request waits on (2412) | a library sample drawn by two accounts is one decode, and a cancel must not turn a request's answer into a 400; the prewarm joins with no threshold, as a request does, so a request that names the stored threshold explicitly has a flight of its own, and a request after a cancel replaces the stopped flight |
| a flight checks the sidecar before it decodes, so concurrent asks on a fresh clip share one read as well as one decode | the first ask's parse answers all of them |
| the gate is the account's: two decodes at once, one under `STUDIO_LOCAL_USER=1`, first come first served (2413) | one account's long file never holds another's click |
| the sidecar is written by `write_atomic` and the memo set after its rename; a failed write fails the waveform (2414) | a reader never parses half a sidecar, a link at the final name is replaced, and an answer without its sidecar would decode again at every ask |
| `check_read` tells a missing, linked, non-UTF-8, unparseable or old sidecar (stale) from any other error, which the route answers and the prewarm counts as failed without decoding (2415) | a transient error is not a stale sidecar; read as one it costs a decode and a rewrite |
| the engine's `--decode` runs under the account's environment alone, is killed at the deadline or the stop whatever it writes, and keeps 64 KiB of its stderr (2416) | a child that writes nothing blocked its reader for ever; the watchdog is a poll of the pipe's reader thread, then of `try_wait` once the output closes, so neither a `read` nor a child that closed its output and kept running is waited on |
| every decode, sidecar read and fingerprint runs on the blocking pool, and nothing recursive runs there (2417) | the runtime's threads answer every account |
| one client, `hyper` over rustls with the front's roots, follows up to 20 redirects by hand, each hop http(s), asks for the identity encoding and refuses a compressed reply; a stalled body fails after 60 s (2441) | `fetch` decompresses and `hyper` does not; no crate is added |
| a download is written to `<dest>.part-<uuid>` created new, counted, checked against the announced length, synced and renamed; a part older than an hour is removed when a download starts in its folder (2442) | the loader opens only final names |
| the destination is made by walking from the deepest folder that exists: its real path must be inside the real models root, then the rest is created (2444) | `create_dir_all` through a linked folder would write outside the root |
| one download per destination in the process: a second account's job waits on the destination's own lock, finds the model there and is done; the module watch is told after each rename (2440, 2445) | two accounts naming one model raced on one part |
| a take and its picture are judged by their real paths inside the record directory, and the directory by its real path inside one of the account's roots; the file that is opened is the real one (2451) | the TypeScript checked lexically, so a link in the record directory uploaded any file the server reads |
| the landing wait is the TypeScript's, 20 s with a look every 250 ms and a size held twice; the take stays in the record directory and the answer is its file name and size; the picture is judged the same way (2452, 2453) | nothing is sent to a library: the project holds its takes |

The tests run the real waveform module and a shell script that stands in for the engine (Unix only), and a raw HTTP server on
`127.0.0.1:0`; the clock of the memos is a value a test turns. The engine's binary and the account's roots are the caller's.

## What the waveform cache and the prefetch decide

| choice | why |
|---|---|
| a sidecar `{source}.cache` sits beside the audio, fingerprinted by content rather than mtime, hidden from the tree and gitignored | it travels with the file and stays valid after a copy; sidecars are not synced, so a clip is analysed once per machine |
| transients are detected once, on the file's own envelope at its native resolution, and stored in file seconds | a short clip resamples up and the detector would run on detail the file never had; two sizes of one audio would disagree about its onsets; seconds survive a change of bucket density |
| a region starts at `sourceFrom + offset` and runs at `warpStretch(warp, tempo)` | the compiler puts a trimmed clip's in-point in `sourceFrom`, and a warped clip covers less of its file per bar; reading less draws a waveform that drifts from the sound |
| a clip's `length` still prices its window 1:1, so a warped clip stops before its `to` and the drawing follows the engine | changing `length` would change what is played |
| the onset sensitivity (2 standard deviations by default) is written into the sidecar and sticky, a batch hands each file the threshold its previous sidecar carried, and clearing the cache resets it | every view of one audio agrees; a rebuilt re-recorded take must not lose a tuned threshold; it is a cache, and a setting that must outlive one moves into the project |
| the tempo is 0 for none, never a guess; the method is an onset autocorrelation over 60 to 200 bpm weighted by a log-tempo prior centred on 120, over a second envelope at a fixed hop | the stored envelope is capped at 8192 buckets and has no uniform time resolution; a hard fold into 80 to 160 breaks both cases it should help, and an unweighted one reads 140 as 70; 174 reading 87 is a known limit of the class because the value is a starting point somebody edits |
| the sidecar's version is required, raised with every field the analysis starts writing | an older sidecar's fingerprint still matches, so without the gate it would never gain the field |
| three layers: a memo keyed by source with the mtime as first gate, a freshness verdict that remembers only YES against both files' size and mtime, then the sidecars; decodes in flight are joined by source and sensitivity and a failed one is dropped | a stale verdict costs a rebuild anyway; a burst arriving during a decode would start a copy of the same decode each, and a failure must not be inherited |
| prewarm decodes a few at a time, one at a time on a local install, and a newer prewarm or a delete cancels by killing the decoders | a decode holds a file's mono signal (57 MB for five minutes) and a phone is killed for memory first; on Windows a held `.wav` blocks a rename |
| the analysis runs off the request thread, with no engine binary through the WebAssembly decoder | the CPU half held every other request, and a hosted service has no engine |
| `low`, `mid` and `high` are optional in the envelope | a surface reading them degrades to a plain envelope rather than an empty one |
| `wf_sig` folds the placement's window for a `whole` draw, the warp markers themselves and the sensitivity and onset corrections, and a cache in front of it folds the same | trimming `from` must miss the memo or the old picture stays; the timeline's own entry per clip is where the bug would come back |
| `whole` means the material: the span is the source's full duration, the window is drawn windowless and shaded from the placement's, with timecode windows resolved on the tempo grid | the window is being judged against the file and must not also crop it; a clip file describes the bytes and a placement the use |
| an id names a placement and the file's properties (onset corrections) are keyed by the material behind it, which a bare material name already is | every placement of a file shares them, and a surface should draw the same onsets without knowing corrections exist |
| the prefetch sends what the open projects name, the audio files their playlists' records point to, one `prefetch` job whose paths the tunnel brings down before the handler runs | a record is a file of the workspace; modules and chains come through the tunnel's `libs` root; the rule that says a file is current is the one every other file obeys |
| the prefetch runs on a timer against the machine bound at that tick and asks again only when the list or the machine changed; a failure is logged once per distinct message | a record changed on the server with the list unchanged is fetched at open, not ahead; a machine that is off would write a line a minute |
| the waveform prewarm is fire-and-forget and `/waveforms/cancel` gives the machine back | a playing surface wants the cores the second decode process takes |
| the waveform route's `sensitivity` is bounded | 0 makes every bucket an onset and a large value none |

## The vectors (2403)

The vectors are frozen, harvested once from the TypeScript's own functions
(`regionWaveform`, `beatGrid`, `buildWaveform`, `wfSig`;
`beatAtSource` and `sourceAtBeat`; `resolveOnsets`; `modelCachePath`; `startModelDownload`'s refusals; `insideDir` and
`takeHere`; `namedSongFiles` over a fixture workspace; the `/waveform` route's clamps through the real router).
The composition of the answer was checked against `WaveformOps.waveform` on a real project when the file was harvested.

| choice | why |
|---|---|
| one section per key, one case per line; about 1.4 MB, not 3.5 | a change reads in a diff, and a case is the smallest one that shows its shape: a grid never reads the envelope, so its envelope has 4 buckets, and an answer names its region by index |
| a number that is not finite is written as its name (`"NaN"`, `"Infinity"`, `"-Infinity"`), on input and on output | the file stays JSON and keeps what the TypeScript answered; the reader turns a name into that f64, and a wire answer writes `null` for it as `JSON.stringify` does |
| the edges are cases of their own: frames from 2^31 (`env.frames \| 0` is ToInt32), the shade's `1e-6` comparisons hit exactly and a hair either side, a shade on a warped and on a stretched clip, a `warpLookups` section over duplicates, dips, reversals and a NaN marker | a bisect that is right on every map a bucket edge happens to probe is still wrong on a duplicate or a NaN |
| the models rows carry no checkout path | the file was harvested with the path shim, not the host's `node:path`, so it holds in any worktree |

Three things the TypeScript answers that the Rust copies: an empty `sensitivity=` clamps to 0.25 and an empty `buckets=`
is 1,000; a repeated `clip=` is joined with a comma and a repeated `buckets=` or `sensitivity=` reads as absent. Two it
does not copy: `buckets=16.5` reaches `new Array(16.5)` in the TypeScript and answers 400 `Invalid array length` once the
region wants that many frames, where the Rust cuts the count to a whole number after the clamp (2427); and a region whose
wanted frame count is NaN (a NaN in-point, or a degenerate warp whose stretch is NaN, which finite text reaches), which the
TypeScript throws on and the Rust draws as one silent column (2428). Both are `error` rows in the vectors, and both are
known differences from node.

## What the memo signature folds (2424)

`wf_sig` is `wfSig`: the fingerprint and everything the region math reads. That includes the time signature's
denominator, which decides the bars, and, for a `whole` draw, the placement's window, which comes from the placement and not
from the material clip the rest is built from. The test holds the signature as a key (the same inputs give the same one, and each input
changed gives another), not only as a string equal to the TypeScript's.

## The routes (`native/media/routes.rs`, `waveform.rs`, `prewarm.rs`, `locate.rs`, `compare.rs`)

`Route::Live` hands `/waveform`, `/waveforms/status`, `/waveforms/prewarm`, `/waveforms/cancel` and `/account/models/:id` to
one hook, which a test drives through `Hook` with a fake locator, and `/models/download` to the same hook.

| choice | why |
|---|---|
| the hook parses a POST's body and resolves the workspace before any route's work, as the live router does | a bad body and an unknown workspace are answered first in node |
| `Locator` is the project side: `locate` for a clip, `project` for the prewarm; its product is the account's song session (the native ask, the web view, `audio_clip_def`, the typed clip-file reader), run on the compiler's stack | the document is the sessions' own, as `docs` answers; a fake stands in for it in the route tests |
| the confinement of a clip's source is `project_roots` over `confine_to`, links followed; a rack's absolute path must lie inside the workspace by the same check (2426) | the TypeScript's check is lexical for the rack |
| the sliced memo is keyed `(project, clip[::whole], buckets)` and its signature is `answer_sig`: the fingerprint, everything the region reads, the sensitivity the sidecar was built with, the corrections (2424) | a rebuilt sidecar or an edited clip file must miss; 16 MiB, least recently used, ten idle minutes |
| a rack's answer is never memoized and has no `detected` or `onsetEdits` | the TypeScript builds it with `buildWaveform` alone |
| the prewarm's answer already says `warming`, `done` and `total`: the stale list is made before the route returns, the decodes go on in a task | the TypeScript sets its status before its first await, and the page reads the answer |
| phase two of the prewarm warms the envelopes' memo and fills no sliced entry (2430, DEPARTS) | the TypeScript's phase-two entries are signed without the sensitivity and the corrections, a signature a request never computes, so none is ever read |
| a cancel is per workspace in its status and in its decodes: the flights record the workspaces whose prewarm joined, a cancel withdraws its own, and a flight stops when no request and no other workspace's prewarm wants it; it answers once they let go, 2 s at most (2431) | the TypeScript aborts only its own run; stopping another workspace's decode makes that prewarm decode the file again as a request no cancel can stop |
| `/models/download` is answered over the session's `model_need` (`song`'s graph read with no compile): `{job: null}` when the model is there, 404 `no node '<n>' in this project`, 400 for a module with no `model_url` in the TypeScript's words, else the job; a body the schema refuses and a graph the session cannot read are refused (2446) | the TypeScript reads `loadGraph` the same way, and the model's arrival is seen at the next ask because nothing is held |
| `/waveform` and `/waveforms/prewarm` read the open project's ask natively (`docs::asked_of`), so they wait behind no verb; an open warms its project's waveforms itself, through the account's `Site` (U2, approved) | the marker names the open project before the open answers, so nothing else is needed |
| a sidecar is written beside the source's real path, not the clip's lexical one: a source reached through a link inside the roots has its sidecar in the link's target folder (2414, DEPARTS, approved U1) | one file has one sidecar and one decode; the lexical name would make two |
| the engine is `machine::engine_binary`, the one `findEngineBinary` is answered by (2402) | a second search would pick another decoder, and the MP3 sidecars differ by decoder |
| `media.saveTake` has no `root` argument: the take's roots are the account's studio and libraries folders, and a test's `landingMs` is ignored (2451, DEPARTS) | `recorderSave.saveTake` is given no workspace, and studio plus libraries contains every workspace |
| with `media` on, a project the native side declines is prewarmed by nobody; the `declines` count says how often (U4, APPROVED) | the other side makes two prewarm owners per workspace whose statuses `/waveforms/status` would merge |
| the prefetch is a timer task of the account's `Media`, started where the engine piece also answers, stopped by the drop (2460, 2461) | it needs the link's jobs address; the first ask comes 5 s after the start and then every minute, each only when the address or the sorted list moved, and a stop drops the job in flight, which the TypeScript never could |
| the open projects are the ones the account's active-project markers name, served by the engine or not (2460, DEPARTS) | the front holds no live workspaces; a marker is what survives a restart, so a project opened and left still counts until another is opened in its workspace |
| one timed loop, a pass after the last one ends, in place of an interval that may overlap (2460, DEPARTS) | two jobs for one list never run at once, which the TypeScript's `running` flag also ensured |
| `prefetches` counts the jobs asked, not those that succeeded; a refused job is said once per message and asked again at the next pass (2461) | a machine that cannot do `prefetch` still shows the front tried, and a list that never settled must not read as done |
| the file list is sorted and without repeats before it is signed (2460, DEPARTS) | two open projects naming one record send it once; the TypeScript would send them twice |
| a pass says its state (`machine=… files=…`) once per change (2460) | an account that prefetches nothing is told apart from one whose machine is missing, which the count alone cannot show |
