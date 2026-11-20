# Declines: what a native piece says when it cannot answer

Every route is answered by a native piece or refused with a sentence (`native/refuse.rs`); nothing falls back to anything.

| choice | why |
|---|---|
| a decline is a refusal with a status and a sentence, never a second implementation | a fallback is a second implementation that must agree with the first; a refusal is a known gap the sentence names |
| `busy` is a 503 with `Retry-After: 1` | the answer exists and costs weight another request holds; the client asks again |
| a hostile folder (too large, too deep, too many entries, a long path, text that is not UTF-8, a path that is no regular file) is a 413 or a 422 with the limit's sentence | the limits protect the server; the person is told which one they met |
| a lost compile is a 503, a shape no native code produces a 500 `internal error` | a retry may help in the first case; nothing a client can change in the second |
| the reason is noted by whoever declines and read back where the refusal is made: `dispatch`, the upload and media streams, the hub's internal routes | one table of reasons and sentences, and no decline site chooses a status of its own |
| the reason lives in a task-local scoped to the request, carried into the tasks it spawns (`refuse::spawn`) | a decline said inside a task a request spawned still reaches the refusal of that request |
| a live route is refused only where a hook owns it (`refuse::live_piece`) | the table is held to the live route list by a test |
| a project that will not compile is a 400 whose body is the song crate's findings worded as the compiler's throw, and a format newer than this build keeps its code and both levels | the page shows the compiler's sentence |
| a docs route with no open project answers the 400 of `requireActive`, the 500 of a route with no try, or the empty view | these are answers, not declines: an idle workspace is a normal state of the page |
| a body a route's schema refuses is a 400 in zod's words (`body::check`) | the words are the schema's |
| `openPrepare` (the libraries, the pins and the sync gate of `/open`), `startEngine` and `push` are native (`open_prepare.rs`, `engine/starts.rs`) | the engine link and the capture are the front's |

## What each reason is answered with

| reason | answer | why |
|---|---|---|
| the 11 of `Declined` (`busy`, `library-active`, `long-path`, `lossy-text`, `no-libraries`, `not-regular`, `surfaces-outside`, `too-large`, `too-deep`, `too-many`, `would-throw`) | a refusal | each names a limit or a state a person can act on |
| `no-active-project` on a docs route | the idle view | a workspace with no open project is a state of the page, not a failure |
| a compile that fails | the compiler's sentence, a 400 on `/project-doc`, `/engine-doc` and `/graph`, the engine load's message | the song crate's findings are the one source of the sentence |
| a chain that does not separate, or both separates and processes | the cut's own 400 sentence | the cut counts a live step as one engine plugin |
| the bench's first pin | written natively | a refusal would leave the bench unpinned for good |
| `hidden-servers`, `lost`, `retired`, `panic`, and the shapes no native code produces | a refusal | a known gap or a failure, never a second implementation |

Tests: one per reason of `Declined` (`native/tests_refuse.rs`), the compile failure and the idle workspace, and the live table.
