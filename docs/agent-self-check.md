# The builder's self-check

Run before `gate.sh --milestone`; the short report lists each item it could not satisfy. Each line is a bug class, a
one-minute check, and a review that found it (in `/Users/mahrave/Src/k7/.claude/state/`); "n" is how many reviews found it.

**Locks and threads**
- **Locks in opposite orders, or held across a send or an await** (n=5). Grep each `lock()` in the diff, list the order per
  path. `step1j-review-J2.md` B1 (binding vs session deadlock), `step1i-review-C5c.md` 1.
- **Work spawned on the actor's thread** (`tokio::spawn` with no runtime; a blocking call or replica lock on an async task) (n=5).
  Grep `spawn`, `.lock()` and `std::fs` in async fns. `step1j-review-J4.md` B2, `step1i-review-C4.md` 8.
- **Work under a lock that a stat would do** (n=3): read or hash inside the critical section. `step1i-review-C5b.md` 2.
- **Ordering lost between two queues** (n=4): one ordered queue per session, not a spawn per frame. `step1j-review-J3.md` B3,
  `step1j-review-J4.md` B3.
- **A cancelled flight reused** (n=2): a request right after a cancel answers "aborted". `step1l-review-L2.md` 2.

**Errors and disk**
- **A transient read error taken as "absent"** (n=3): grep `Err(_) =>`, `.ok()`, `unwrap_or_default` on I/O.
  `step1g-review-G0-G1.md` F2, `step1i-review-C5b.md` 1.
- **Memory ahead of the disk after a failed write** (n=3): fail each write, check the head. `step1i-review-C5c.md` 2 and 3.
- **A path, count or error code that differs on Windows** (n=3): `cfg(unix)` locals unused, a test vacuous there.
  `step1k-review-K1.md` B1 and 9, `step1g-review-G0-G1.md` F4.
- **An engine run writing the user's real settings** (n=2): scratch `HOME` and `STUDIO_HOME`, compare `Settings.xml`'s mtime
  before and after. `step1m-review-M0.md` 2.

**Tests**
- **Fails only through a disk assert or a time bound, not a count** (n=6): revert the fix, see which assertion fails.
  `step1i-review-C5c.md` 4, `step1g-review-G4.md` B1 (the twins' own clock).
- **A mutation the brief names, never shown failing** (n=6): run it, keep the list in the state note.
  `step1k-review-K1.md` 10 and 11, `step1g-review-G2-G3.md` F14.
- **A deadlock test that hangs instead of failing** (n=2): every wait carries a short timeout that panics.
  `step1j-review-J3.md` B4, `step1k-review-K3.md` 3 (a 10-minute hang).
- **A test bigger than its proof** (n=3): 64 MiB where 4 MiB proves it. `step1k-review-K2.md` 2, `step1k-review-K1.md` 6.
- **A hostile row of the brief with no test** (n=4). `step1l-review-L2.md` 5, `step1j-review-J2.md` 10.

**Vectors and figures**
- **Vectors carrying the builder's own path** (n=3): imports before the path shim, a depth of `../`; regenerate under
  another `TMPDIR` and in a second worktree, and see the replay pass. `step1g-review-G0-G1.md` B1, `step1m-review-M0.md` 1.
- **Figures on a project that does not compile, or with no verb's own time** (n=3): a golden demo, one verb timed.
  `step1k-review-K0.md` decision 5, `step1n-review-N0.md` F1, `step1g-review-G0-G1.md` F6.
- **An op name colliding in the bundle's table** (n=1): grep the new name in the one operation table. `step1j-review-J3.md`.

**Spec and honesty**
- **The TypeScript's quirk copied without a page row** (n=3): each copied oddity has its row. `step1k-review-K2.md` 4,
  `step1k-review-K1.md` decision 4.
- **A departure taken without asking** (n=4): every difference from the brief or the TypeScript is listed.
  `step1l-review-L2.md` 1, `step1g-review-G2-G3.md` F11.
- **A page or comment now false** (n=4): grep the page for what the diff changed. `step1i-review-C5b.md` 4, `step1l-review-L1.md` 3.

**Commands**
- **`--target-dir` after `--`** builds in the tree: it goes before. `docs/agent-review-template.md`, "How it works".
