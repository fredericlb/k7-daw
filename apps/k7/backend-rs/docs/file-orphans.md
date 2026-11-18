# The orphan reads and the account's preferences

The routes no earlier piece owned answer natively: `/module-ui`, `/module-base`, `/module-engines`, `/consoles`,
`/project-plugins`, `/project-compat`, `/resources`, `GET /preferences`, `PUT /preferences/vim`,
`PUT /account/cold-chains/:which`, `/graph?at=`, and the shares' `songUuid`.

| choice | why |
|---|---|
| the module reads sit on `k7-file-service/src/module_reads.rs` over song-edit's loader and answer with the `entries` piece | they read what the module routes read; a second piece would only add a state nobody sets |
| a read takes the open project from the workspace's marker (`docs::asked_of`), as `/presets` does | the open writes the marker before it answers, so the marker is what every native reader names |
| a refusal is the TypeScript's sentence in a 400 (404 for the two project-need routes) | the page shows the sentence; goldens from a real `Workspace` hold it |
| the goldens are `moduleReads`, `projectReads` and `preferences` under `k7-file-service/tests/goldens/server/`, `moduleReads` and `projectReads` made by `packages/song-edit/scripts/{moduleRead,projectRead}Goldens.mts`, `preferences` frozen (its TypeScript upgrade is gone) | the replay compares the answer and the folder after, files included |
| the legacy `preferences.yaml` upgrade is ported whole (`preferences/upgrade.rs`): the lock file, the exclusive create, the read-back, the take-back, the first free `.pre-k7x` backup | a YAML is never deleted and a save that landed in between is never lost; the vectors include another process holding the lock |
| the YAML is read by a small saphyr loader of the core schema (`preferences/yaml.rs`) | a preferences file is a flat mapping; a repeated key or a second document does not read, as `yaml` refuses them |
| `/graph?at=` draws the folder through the song session with `Folder::Other` | a library chain's graph has no freezes of its own |
| `songUuid` is a staged write landed whole | the share's song id is written once, before the project is copied |

## Departures

| what | the TypeScript | here |
|---|---|---|
| a clip file that does not read, in `/resources` | the route throws its syntax message (a 500) | the clip is left out |
| a project the graph refuses, in `/project-plugins` and `/project-compat` | the throw's text in a 404 | the first finding's text in a 404 |
| a lock whose holder died | broken at once on this host, by `kill(pid, 0)` | the same on unix; elsewhere after ten seconds |
| a YAML with tags or merge keys | read by `yaml` | tags are ignored and merge keys are plain keys |
