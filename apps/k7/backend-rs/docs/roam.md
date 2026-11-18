# The list of workspaces and the iCloud container

What roams between a person's Apple devices besides git: the list of the workspaces they added, and, once, the move of the
workspaces an earlier version left in the iCloud container. The decisions of what travels how are
`docs/project-sync-plan.md`; the Apple side is `engine/k7-runtime/docs/ios.md`. The crate is `k7-roam`, the route is
`routes/files/service/native/git_roam.rs`.

## Choices

| choice | why |
|---|---|
| the store is a trait (`ListStore`: pairs, put, available) with `NoStore` as the default and `Fake` for tests | Android, Windows and Linux have no list; the route answers `available: false` and the page shows nothing |
| the host's store is two C function pointers (`k7_roam_install`), not a Capacitor plugin | the page reaches the list through the front's own route, so no second path (page to Swift) exists, and the Mac's iPad app and the phone share one mechanism |
| the callbacks list the pairs through an `emit` function the Rust side gives them | no string crosses the boundary owned by the other side, so nothing is freed twice |
| one key per workspace, `ws.<id>`, the value a JSON text | the store is last-writer-wins per key; two devices that each add a workspace never overwrite each other |
| an id is a hash of the normalized remote (`git@h:o/r.git`, `ssh://git@h/o/r` and a trailing slash are one) | two devices that cloned one repository share a key; any spelling of the address gives the same one |
| a workspace with no remote has a random id kept in `.k7-roam.json` in the account's folder, beside the identity | only a remote can match a workspace across devices, so the id is the device's |
| `.k7-roam.json` maps each name this device listed to its id | a name in it whose folder is gone is a removal, and an id that changed (a workspace just connected) leaves a tombstone on the old key |
| a sweep writes an entry only when the list has none or its name or remote differ, and never over a tombstone | a device that still has a workspace another device removed does not bring it back at its next start |
| a tombstone is dropped after 30 days, at a sweep | a device offline for longer reads a removed workspace as live; thirty days is the price of a store that never grows |
| the sweep runs on every read of the route, after a connect and a clone, and after any write of the workspaces' routes, on a blocking thread | libgit2 reads each workspace's remote; the answer to the page never waits for the writes after an act |
| Examples is never listed | every device has its own copy of the demos |
| the route answers `{available, entries, here}`: the live entries, and the ids this device holds | the page decides what is offered (a remote, not here); the host stays free of screens |
| the container is moved out at an account's start, before Examples is stocked, from `K7_ICLOUD_LEGACY` (the container's `Documents/`, set by Swift only when it holds anything) | the account's folder is known there, and Examples then lands in the folder the demos are mirrored into |
| a workspace moves by `rename`; when that fails it is copied into a scratch folder beside the target, and compared (the same entries, the same sizes, links as links), and a git repository must read every object, then it is renamed into place and the original deleted | a rename across volumes fails; a half-copied workspace must never take the name of a whole one |
| a name already in the folder, a file that is a placeholder (`.<name>.icloud`) or dataless, and a failed copy or check each leave the container's folder alone and say so in the log | the person's data is deleted only after its copy is proved, and never replaces another |
| a container copy that cannot be fully removed after a good move is reported as left over, and the next start finds the name taken and touches neither | two copies are the worst case, and neither is damaged |
| the capture's markers (`.active-projects/<name>`) move one by one, only where the device has none of that name | a marker is the device's own record of which project is active |
| a `.git` that is a link to `.git.nosync` is made a real folder again, on any open and in the move (`k7-git`'s `gitlink.rs`) | a workspace converted by an earlier version must open as an ordinary repository |

## Tests

`k7-roam`'s unit tests cover the encoding, the id, the tombstone rules, the sweep and the callbacks; `tests/container.rs` the
move, with a seam that fails a rename, a file's copy or a removal; `k7-git`'s `tests/gitlink.rs` the link; the route's test is
`tests_roam.rs` in `k7-front`.
