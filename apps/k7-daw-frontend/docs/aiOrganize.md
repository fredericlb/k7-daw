# AI organize in k7: the conversation is the page's, one turn is the server's

The module builder's **AI organize** (`packages/ui-module-builder/docs/aiOrganize.md` says what it does and
how the answer is judged) needs a host for two things the builder cannot do: spend an account's Claude key,
and keep that key from every page. k7 splits the work on that line. The page runs the conversation and every
check (`apps/k7/frontend/src/api.ts` calls `organizeLoop` of `@studio/panel-rules/ai-organize-loop`); the
front's `POST /api/ai/organize/turn` makes **one** model turn with the account's key.

## The choices

| choice | why |
|---|---|
| the checks and the repair round run in the page, over the same TypeScript the builder's grid uses | `@studio/panel-rules` stays the one place the rules live; a second implementation on the server would have to agree with the first on every rule the grid enforces |
| `widgets.js` is compiled by the page's own `new Function` and never called | a JavaScript syntax check needs a JavaScript engine, and the page has one; the model's script has nothing to run in beyond the page that is about to show it anyway |
| the server's turn takes the prompt, the answer's schema and the conversation, and decides none of them | what to ask is the page's; the key, the model, the output limit and the effort are the account's and the server's, so a page cannot choose a dearer model or a longer answer |
| the turn names no panel rule | the server checks the SHAPE of what it forwards (a prompt, a schema, up to five messages that start and end with the user), not its meaning |
| a message's blocks are text, images and the model's own thinking, nothing else | the turn is the account's key spent for the page: a document, a tool result or a URL image would make it a general-purpose proxy |
| the model reads module and plugin names a shared library can carry, and `widgets.js` is saved, then run by the panel runtime | a hazard kept in view, not closed: the page's compile check runs nothing, and a library's names are as trusted as the library |
| the model is the account's (`claude-opus-5` when none is named) and `max_tokens` is 64,000 | the price of a run is the account's to set; a body's own `model` or `apiKey` is not read |
| one turn answers as NDJSON: `status` lines while Claude writes, a `ping` or the last status again every 15 s, then exactly one `turn` or `error` | a turn with thinking can take over a minute, and a proxy may cut a silent socket; closing the tab drops the request to Claude rather than paying for a panel nobody will see |
| the answer is cut at 8 MiB of stream | a real answer is bounded by `max_tokens`; an upstream that never stops costs one call, not the server's memory |
| a refusal and a cut-off are the server's errors, a mistaken panel is the page's | the server reads `stop_reason`; whether the text is a good panel is the loop's question |
| the page checks the answer again where it lands in the buffers (`useAiOrganize.ts`) | the check is cheap, and the buffers are where a wrong panel would do harm |

## What proves it

The two halves are held by:

| proof | covers |
|---|---|
| `packages/panel-rules/src/aiLoop.test.ts` | sixteen cases: the same model replies draw the same turns (prompt, schema, repair message) and end the same way; a model's junk, a broken `widgets.js` and a request that is no request |
| `apps/k7/backend-rs/crates/k7-front/tests/all/ai_turn.rs` | the guards in their order (session, key, size, shape), and that only the account's key and model reach Claude |
