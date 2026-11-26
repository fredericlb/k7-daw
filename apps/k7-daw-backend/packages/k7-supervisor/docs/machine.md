# What a server asks of the machine

The jobs of a jobs channel are a table (`tunnel.md`); this page is the choices of the ones that are not the cut
(`separation.md`) and not the slots (`slots.md`). There is no `local` job: nothing asks it.

| choice | why |
|---|---|
| one module per job in `parts/tunnel/` (`log`, `files` for `prefetch` and `send-up`, `settings` for `onnx-providers` and `chains`, `slot_jobs`, `render`), the work apart from the table in `machine/` | the seam stays one line per job, and what is not a path check (a slice of a log, a render, a purge) is plain code the tests call without a core |
| `onnx-providers` and `chains` write through `Ctx::update_settings` and answer once the change is seen (5 s at most: a loaded core answers late, never wrong) | the core is the file's one writer; the reply is the settings as the core then holds them, so the studio's card shows what the machine will use |
| `log` reads at most 1 MB of the merged log per call; a line longer than that is skipped | a reader far behind catches up over several polls, and a line that never ends must not hold it in place |
| the render's scratch folder is made by the job, owner-only, with a random name, and removed however the job ends | a name that can be predicted is a folder another user of the machine can make first or point elsewhere; a computer's lives in the temp folder, an app's under `<state>/render-tmp`, the one place the engine may write |
| a computer renders by running `k7-runtime --render - --out <scratch>` with the doc on stdin; an app (`engine::IN_APP`) asks the running engine over a socket of its own for the whole render, up to 2 hours | an app may start no program; the engine answers `rendered` to the asking socket alone, so a render outlasts any query's deadline |
| the manifest's file names must be children of the scratch folder | the engine names the stems, and a name climbing out of the folder would copy another file; each destination is also held to the job's writable folders |
| the renders in flight are the snapshot's `rendering`, one counter for every agent, kept by the tunnel part | the app's Busy reads it; a counter per agent would overwrite another's patch |
| the render binary is found when the job runs (`$STUDIO_ENGINE`, then beside the supervisor, then the repo's newest build) | the same search as the engine's start, so a build that appears later is used |
| kept undo audio is purged at the start and every hour by the tunnel part, only under `history-*` roots, links never followed | a server asked this machine to keep it a month past its last use; a workspace copy is the tunnel's cache, not the purge's |

## How it is tested

`tests/all/machine.rs` holds the log's cases, the purge, and the render through a script standing for `k7-runtime` (Unix)
and through a WebSocket server standing for the engine; `tests/jobs_part.rs` asks the jobs over a hub in the whole core.
