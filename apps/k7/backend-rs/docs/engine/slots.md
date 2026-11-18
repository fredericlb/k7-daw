# An engine's slots, set from a server

The routes under `/api/engine-slots/:id` let an engine's owner list, save, enable, disable and remove
the slots of a machine whose supervisor holds a tunnel to this server
(`apps/k7/backend-rs/crates/k7-front/src/routes/engine_reach/engine_jobs.rs`). What a slot is, and the machine's rules for keeping one, are
in `engine/k7-supervisor/docs/slots.md`. The page that calls these routes is described in
`apps/k7/frontend/docs/slots.md`.

| choice | why |
|---|---|
| only the engine's **owner** may (`engineRole`), a share is refused | a slot is what the machine plays with nobody attached, a setting of the machine and not a session on it |
| the chain is compiled here, by the account's file service (the file hub's `POST /account/slot`) | the chain and its files are the account's files on this server; the machine may hold none of them |
| the slot goes to the machine as a job on a `jobs` channel, opened with the account's roots and the libraries' | the tunnel downloads every file the job names under those roots before running it, so the copied slot holds only what crossed |
| a save waits up to 10 minutes, with no shorter idle limit | the machine replies only after the files have crossed, and an SFZ's folder can be large |
| an engine that is registered but holds no tunnel is a 503, saying so | the slot cannot be set from here at all, and that is different from a refusal |
| a compile refusal is a 400 with the file service's reason; the machine's refusal is a 400 with its own | "this chain cannot play live" and "this machine lacks the plugin" are both things the owner can fix |
| every answer is the machine's store as it then stands, `{slots, enabledSlot}` | the page redraws from it with no second request |
