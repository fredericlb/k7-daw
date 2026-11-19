# The exports, held natively

The mixdown `/export` streams, the tracks zip `/export/tracks` and the picture render's job, all in the front
(`k7-front/src/routes/files/service/native/shares/`). The `picture render` verb and the k7x write of a new song's
`uuid` are the verb host's.

## What the vectors hold

`tests/vectors/shares.json`, frozen, harvested from the TypeScript's own code with no generator kept:

| section | cases | what it pins |
|---|---|---|
| `export` | 37 | `/export` through the real route: the name, the `Content-Disposition` (ASCII `filename` and RFC 5987 `filename*`), the refusals, a repeated or structured `timeline` refused |
| `queries` | 64 | a query read as Express reads it |
| `settings`, `job` | 21, 16 | `resolveVideoSettings` including a zero length and a height over a zero viewport; the job's text over scenes whose numbers serde would respell |
| `chromium` | 23 | `findChromium` over virtual trees: revisions in numeric order, every layout, every root, the system's candidates |

## What the crate's model holds

`k7-file-service/src/shares/` is the pure side, replayed against the vectors by `tests/all/shares_*.rs`:

| file | choice | why |
|---|---|---|
| `names.rs` | `timeline` and `format` are read from the raw query as Express reads them: repeated, `[]` and `[k]` forms | a repeated `timeline` is refused, not read as Main (2520) |
| `qs.rs` | a query key written `[timeline]` is read as `timeline`, as qs reads it | `?[format]=mp3` exports MP3 in node, and the vectors come from express 4.22.3 with qs 6.16 |

## Why the picture is the same twice

A film is the page's work over the job, the media and the mixdown, in one browser. Each source of difference
is pinned rather than tolerated:

- **The browser and its flags**: one binary for both renders in one run, `k7-picture`'s fixed flags (`--lang`,
  `--force-color-profile=srgb`, SwiftShader where the browser ships it). No film is committed: a committed
  film would hold one machine's fonts and one build's encoder.
- **The job**: the scene's text is spliced as written. A number re-spelled by serde (`6.0` for `6`, `1e21` for
  `1e+21`) would be another job. Tests compare the job's text without any browser, so a job difference is found
  in seconds rather than through a film.
- **Time**: WebM carries none. mediabunny's MP4 muxer stamps `mvhd`, `tkhd` and `mdhd` with `Date.now()`; those
  fields are zeroed before an MP4 is compared, and nothing else is masked.
- **Random numbers**: mediabunny draws them only for an attached image's UID and a generated name, neither of
  which a film carries; measured identical.
- **The sound**: an audio clip renders the same `data` chunk twice, a synth never (1k's 2323). A picture with
  sound is compared over an audio-clip project only; a synth's picture is compared silent.

The tests never use the person's browser profile: `k7-picture` starts every render with a fresh
`--user-data-dir`, and the harness names Chrome for Testing explicitly and refuses the system's Chrome.

## What the front's I/O holds

`k7-front/src/routes/files/service/native/shares/` is the I/O side, over ports (`Mixdown`, `Views`, `Browser`) that the
account's parts fill in; its tests stand fakes on them.

| choice | why |
|---|---|
| a mixdown lives in a `Folder` that removes itself when dropped | a client that leaves, a failed render and an unread body all end in the same cleanup (2522) |
| the film is written to a dot-named part file beside its destination and renamed | a walk of `exports/` must not count a half film (2505); the brief's `<out>.part-<random>` is dot-named for that reason |
| the scene is parsed only to read three numbers and to walk for `media/` strings; its depth is serde's own bound | the job is spliced from the text, so a scene deeper than the bound is refused rather than recursed into (2504, 2552) |
| a second picture render is refused with "a picture render is already running" | the TypeScript has no sentence of its own for it |
| the Fetch gate covers the page's own requests, not a worker's | a dedicated worker has its own DevTools session; the render page starts none |
| the live gate test runs with `K7_LIVE_CHROMIUM` and returns at once without it | a browser is not a dependency of the gate; the milestone run named Chrome for Testing 156 |

## The tracks zip

`/export/tracks` is a second route of the same piece, answered from the same `Route::Live` branch and gated like
`/export`; its own page is [tracks-export.md](tracks-export.md).

## What the routes and the ports hold

`routes.rs` answers `/export` and `/export/tracks` from the `Route::Live` branch; `ports.rs` binds the
traits to the account's own parts. The piece needs `docs` and `engine-jobs` answering.

| choice | why |
|---|---|
| each account's `Shares` lives in a registry keyed by its state folder (`registry.rs`), kept for the life of the process | a picture render in flight outlives a rebuilt `Held`, the registry does not |
| the counts of the `[files] native shares:` line (`exports`, `pictures`, `declines`, `panics`) are one set for the whole front, with one reporter, started once beside the counts | the bed's judge reads the last such line, so a count per account would show one account's work; a reap must not reset them, and a change just before the last reap is still logged |
| the mixdown and the compile go through `Jobs::ask`, `Jobs::query` and `docs::prepare_with`, never a driver or a session of their own | one render path and one compiler answer |

## Why the browser fetches only its job

The page is a browser on the server, and a scene is built from song files a person writes. Any URL that reaches
the page (a shader's, a scene field's) would be fetched by the server: its own front on the loopback, a private
address, a cloud's metadata service. `k7-picture` therefore fails, through the DevTools `Fetch` domain, every
request outside the job's own path. 

## What the bed holds

| choice | what forced it |
|---|---|
| the bed judges the last `[files] native shares:` line (`bedCounters.mjs`): `exports` 2, `pictures` 1, `panics` 0 | a run that exported nothing must not read green; a filtered run reads below the floors, so only the whole filter is judged |
| `serviceMediaVerbs` renders a 160x90, one-second film | a full-size film never ends in the Mac's software GL, with the piece on or off, so the picture floor was unreachable |
| `declines` counts the requests the render page was refused (the Fetch gate), shown and never judged | a bounded page asks for what it may not, and that is the gate working |

## Risks

| risk | answer |
|---|---|
| the server's browser used as a fetcher | 2506 |
| chromium's DevTools port reachable by every local process while a render runs | accepted; raised for desktops |
| a film, a mixdown or a `uuid` written through a link | 2556, 2522, 2531 |
| the engine's `--render` writes `~/Library/Studio/Settings.xml` whatever `HOME` says (measured) | harness runs kept few, said in each report; the engine's settings folder comes with `K7_SETTINGS_DIR` |
| the films differ for a reason outside the job | the table above; the job's text compared first |
