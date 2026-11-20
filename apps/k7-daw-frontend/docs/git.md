# A workspace under git: the screens

The home page connects a workspace to a git remote or clones one, and a bar over the workspace shows where it stands
against the remote. The routes are `apps/k7-daw-backend/docs/files/git.md`, the repository and the merge are
`apps/k7-daw-backend/docs/git.md`; this page covers what the screens decided. The code is `src/git/`.

## Choices

| choice | why |
|---|---|
| the bar branches on the route's `state` alone, through one function (`barView`) | the routes own the rules of what may go where; a screen that rebuilt them from `ahead`, `behind` and `changed` would drift |
| Push, Pull and Commit are the person's acts: nothing is pushed, pulled or committed by itself | a take or a song is a unit of work the person names; the fetch only looks |
| Commit & Push is one act, shown as a step list (Committing, Pushing, Done) inside the dialog | a commit nobody pushes is a step the person had to remember; the list says where an act stopped |
| a push the remote refuses as `behind` is the dialog's own state, "Pull first", with the commit kept; any other failure is the failed step with Try again, which resumes at that step | the refusal is the next step, not an error, and a failed push must never commit twice |
| with nothing to commit and commits waiting, Commit & Push only pushes; a workspace with no remote only commits | the act does what the workspace can do, so a pull that was merged can still be sent |
| Pull over uncommitted changes reads "Commit first" and is dimmed | the sentence says the next step, and the server's refusal (`uncommitted`) is the same sentence if a stale bar lets a click through |
| Abort asks inside the Conflict dialog, as a nested `dialog "Abort the merge?"`, not as a popin | a popin over a modal is drawn under it and cannot be clicked |
| the remote dialog has no HTTPS / SSH choice | k7 is ssh only: a choice with a refused option would only be a trap |
| the command editor is a full-screen editor; its Save writes the whole draft, and a key taken from another command is said under the filter row | a shortcut saved one at a time would half-apply when the person leaves |
| a merge waiting shows only its own button: no Commit, Push or Pull | a commit during a merge would be a second history |
| a pull that diverged opens the merge screen at once | the person was waiting for the answer to the pull |
| the Conflict screen is a list of conflicts (marked "To choose" until picked) over the open conflict's cards: Mine, Theirs, Both only where the route says, keep or remove for a file one side removed; "Next conflict" walks the unchosen ones | "both" has a meaning only for a keyed list item, or for a binary kept beside under a free name, and the screen never offers what the merge would refuse |
| "Mine for all" and "Theirs for all" skip binaries and removed files | those are the choices a person should look at |
| the choices are sent once, on Finish merge, and Finish is allowed when every conflict has one | nothing moves in the workspace before the person is done; Abort leaves it as it was |
| the editors are read-only while a merge waits, through the shell's write gate and `locked` | the file service answers 423 `merge-pending` to the same writes; the gate says it before the request leaves, and the engine plays on |
| a failed state read keeps the last state; only a missing route (404, 501) clears it | a blip must never unlock the editors of a merge |
| the home page asks each workspace's state every 15 s to refuse its own writes while a merge waits (`useMergingWorkspaces`) | the home page has no bar of its own to keep the state |
| a commit takes the whole tree: the dialog lists the changed files with their sizes and marks large files, it does not select | what is committed is what the person sees, and a partial commit would leave a tree no one has tested |
| a file the status flags `tooLarge` (over 2 GB) is left out of the commit, and a line names it with its size; the dialog sends it as the commit's `exclude` | the host would refuse it, the person's file stays where it is, and the next commit leaves it out again until it is shrunk or removed |
| the state's `changed` and the dialog's "nothing to commit" ignore a `tooLarge` file | a file the commit cannot take must not look like a change waiting |
| a commit asks for a name and an email when the device has none, and keeps them (settings, Devices) | git needs an author; it is per device because two machines of one person may sign differently |
| the dialog has an address and no sign-in field: no token, no password, no user name, no pasted key | k7 is ssh only; the address carries the user (`git@host:…`) and the device's own key signs in |
| an `https://`, `http://` or `git://` address is said at once ("k7 syncs over SSH only") and cannot be sent; any other address goes to the route, whose `ssh-only` answer is the same sentence | the page names the way on from the first keystroke, and the route stays the one judge of what is an ssh address (the e2e bed's `file://` seam passes) |
| the dialog shows the device's public key in a read-only box with a Copy button, and one line on where to add it (a deploy key, or the account's SSH keys) | the person's one step on the host; the key is the same for every workspace, so it is made by the device on the first open of the dialog |
| Copy tries the clipboard API, then selects the box and runs `execCommand("copy")`; a failure leaves the key selected and says to copy it by hand | a webview may refuse the async API; the selection is the way every webview lets a person copy (`copyText.ts`) |
| the first call carries no `hostKey`; a 409 `unknown-host` shows the fingerprint and asks, and the call is made again with `hostKey` once the person confirmed it | trust on first use is the person's decision, and nothing was sent to a server nobody vouched for |
| a confirmed fingerprint belongs to the address it was shown for: editing it asks again | a fingerprint carried to another server would trust a key nobody looked at |
| a 409 `host-key-changed` is a warning with both fingerprints and no button to go on; its one way on is "Forget the old key", after which the next call is a first contact | a reinstalled server and a machine on the path look alike, and the dialog cannot tell them apart for the person |
| a host without large-file storage is said after a connection by a dialog, and a push to it shows the server's sentence in the bar | the person can still read and commit; only the push is refused |
| the page tells the server it is in front (`focus`) with each state poll while the tab is visible, and away when it hides (`focusBeat.ts`) | the fetch runs at open, every 5 minutes in front, and when the page returns; it never runs for a tab nobody looks at |
| the demo workspace (`Examples`) offers no "Connect to git…", and the connect route refuses it | the demos are put back as shipped at each start, so a history there fights the reset, and a push would send the demos into the person's repository |
| the workspace menu reads the state once when it opens; connected, it says "Connected to host/repo" (host and repository only) and offers "Change remote…" in place of "Connect to git…" | a plain Connect over a connected workspace would read as a second repository; the same dialog re-points `origin` |
| a connect ends with a toast naming the workspace and the remote, and announces on `gitBus` | the person must see where they clicked that it worked, and every other reader of that workspace (the home row) reads again at once |
| a workspace's home row carries a badge: the branch, the bar's counts and Commit, Push and Pull, read once at mount and on an announcement, never polled | the bar's pieces (`BarBody` in its `row` form) keep the rules in one place, and a home page of many workspaces must not poll each of them |
| the home page's "On your other devices" lists the workspaces added elsewhere that have a git remote and are not here, each with a Clone; one with no remote is not shown | a workspace needs a git remote to travel, and the clone is the clone dialog opened with the address and the name filled in, so the host-key question and the name's collision check are the ones the person knows |
| the list is read when the page opens and every 20 s while it is visible, a failed read keeps the last answer, and a cloned entry hides at once (`roamState.ts`) | the section is a convenience and never a gate; the person must not see the entry they just cloned for the length of a poll |
| History has two lists: Edits (the local history, as it is) and Commits (the log, older pages on demand) | an edit is a step of this device; a commit is what was named and shared |

## Where each screen is

| screen | place |
|---|---|
| Connect to git | the workspace's menu, "Connect to git…" |
| Clone from git | the home page's header, beside "New workspace" |
| On your other devices | the home page, under the header, only where the host has a list (iOS) |
| the bar | above the project bar in the editor, and above a workspace's tree on the home page |
| Commit & Push and the Conflict screen | dialogs opened through `gitScreens.tsx` (`openCommitAndPush`, `openConflict`), by the header and the workspace card |
| Commits | the History panel's second list, shown when the workspace is under git |
| the commit author | settings, Devices |

## Tests

`gitState.test.ts` holds the rules of the bar as data; `connectState.test.ts` (the address, the trust, the refused web addresses), `copyText.test.ts` (the clipboard and its fallback), `mergeState.test.ts`, `commitState.test.ts` and
`logState.test.ts` hold the reducers and their refusals; `roamState.test.ts` and `OtherDevices.test.tsx` the list's reducer and the section; `gitLock.test.ts` the gate that keeps the editors still, with its wiring
into `App.tsx`; `GitBar.test.tsx` renders the bar and the cards. The functional spec is `e2e/specs/app/git-sync.spec.ts`.
