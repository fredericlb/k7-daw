# The account's runtime state (`Held.runtime`)

An account holds no thread and no JavaScript runtime. What the Logs panel, the background jobs' list, the module watch,
the bound machine's asks need is `Held.runtime`: the Logs ring, the background
registry and the machine asks. This page holds the choices and what forced them.

## The Logs panel's volume

| measure | figure |
|---|---|
| a bed's conformance run (443 specs in 2.5 minutes, some 20 accounts) | about 41 lines a minute in all; 120 B on average, 325 at most |
| the ring | 1,000 lines per account |
| the panel | polls every 1.5 s while it is open, `tail` 300 when it has no cursor: about 36 KB once, then only the new lines |

## What the service's own log decides

| choice | why |
|---|---|
| the account's own lines are kept as numbered, levelled lines in a buffer of the last 1000, beside the engine machine's log, and a cursor higher than anything here restarts from the start | the Logs panel has two halves; a cursor from a previous process must not answer nothing for ever |
| the service log is the account's, in the front, fed by the front's lines and every native piece's | node's panel showed everything its one process said |
| the machine's half of `/logs` is asked over the jobs path natively, bounded at 5 s and at two asks in flight | a hung machine must not hold the panel |
| one account per ring, so a panel shows its own account's lines and nobody else's | a ring of two accounts would leak lines |
| a key repeated in `/logs`' query is one value joined by commas, so `?after=3&after=4` is absent and the tail is 300; a key present and empty is 0 | Express's array reads so under `Number` |

## The rows

| # | as built | held by |
|---|---|---|
| 2604 | `tests/vectors/runtime.json`, frozen: `num` and `tail` over 31 query spellings each, `since_small` and `since_big` over rings of 0 to 1,500 pushes, `label`, `processes` (10 scripts on a turned clock) | `runtime_vectors` |
| 2620 | the ring is `runtime::ring::Ring`: 1,000 lines, a text cut at 16 KiB on a character boundary and ending ` …`, a cursor past `seq` answering from the start | `runtime_ring` |
| 2622 | `/logs`' numbers are `runtime::logs`: JavaScript's `Number` over a query string, the tail clamped to 1..2000 and used as its integer part, the answer's keys `offset`, `lines`, `service` | `runtime_logs` |
| 2630, 2631, 2614 | the registry is `runtime::background::Jobs` over `runtime::list::Table`: one job per tag, listed at once, kept 8 s after its end, held by a 10 minute lease that any message renews, a drop under jobs ending each `failed: the file service restarted` | `runtime_background` |
| 2633 | DEPARTS. A progress told after a job's end is dropped; node wrote it onto the ended entry, which then listed a bar beside `done` | `runtime_background` |
| 2670, 2671, 2672 | the front's stop is `Services::stop`, awaited: it refuses to make an account, stops every held account in parallel under one 5 s bound, each member bounded at 1.5 s, in registration order (`Held::members`), and says one line naming the accounts and members left; the reap runs the same `Held::stop` with no bound | `runtime_stop` |
| 2680 | the front logs `[files] native runtime: background=<n> logs=<n>` every 30 s while it moves; `scripts/agent/bedCounters.mjs` judges it with a floor `logs>=1` | `bedCounters.test.mjs` |

## Why the stop waits

The front's stop waits for every account, bounded: a ledger saved after the process left is no ledger, and the bound keeps a hung member from holding a quit.
