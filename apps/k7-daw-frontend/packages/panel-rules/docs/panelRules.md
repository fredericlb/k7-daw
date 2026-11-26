# The panel's rules, apart from the builder

`@studio/panel-rules` holds what decides whether a module's panel is valid — a group frames what
lies wholly inside its box (`src/groupGeometry.ts`), the horizontal and vertical layouts offer the
same controls (`src/layoutTwins.ts`) — the one way a panel is printed as `ui.k7x`, a widget a line
(`src/panelText.ts`), and AI organize's wire, answer, check and system prompt built on them
(`src/aiOrganize.ts`, `src/aiAnswer.ts`, `src/aiPrompt.ts`). What the rules ARE is the builder's to explain:
`apps/k7-daw-frontend/packages/ui-module-builder/docs/builder.md`, `layouts.md` and `aiOrganize.md` there.

## Why a package of its own

Two things apply these rules: the builder's grid, and AI organize's loop (`src/aiLoop.ts`), which checks a
proposal from Claude in the page before it reaches the buffers. The builder is React, CodeMirror and Radix; a
test or a server taking the check from it would carry all of that, for some 850 lines of pure functions.

So the rules have no React and no DOM, and depend on the file format (`@studio/song-schema`,
`@studio/k7-wasm/k7x`) and nothing above it. The builder imports them like any other host, and still answers
`@studio/ui-module-builder/ai-organize` for the frontend that reaches AI organize's types
through it.

A rule the grid enforces belongs here when the AI check must enforce it too; one that only the
editing gestures care about stays in the builder.

## The loop

`src/aiLoop.ts` is AI organize's conversation, run in the page (`apps/k7-daw-frontend/docs/aiOrganize.md`): it checks
the request (`src/aiRequest.ts`), asks the host for a turn, reads the answer (`readAiAnswer`), checks it
(`checkAiResult`) and hands the mistakes back once. The host's turn is a function the loop is given, so the
vectors of `organize-vectors.json` drive it in a test (`src/aiLoop.test.ts`) with no server. A model's junk (text that
is no JSON, JSON of the wrong shape, a script that does not compile) is a refused answer, never a crash.
