# AI organize in k7: the page asks once, the host holds the conversation

The module builder's **AI organize** (`apps/k7-daw-frontend/packages/ui-module-builder/docs/aiOrganize.md` says what it does and
how the answer is judged) needs a host for two things the builder cannot do: spend an account's Claude key,
and keep that key from every page. k7 puts the whole conversation on that side of the line. The page sends one
request and draws the stream (`apps/k7-daw-frontend/src/api.ts`, the reader in `@studio/panel-rules/organize-stream`);
the front's `POST /api/ai/organize` (`apps/k7-daw-backend/packages/k7-front/src/routes/ai/organize.rs`) asks the
model, checks the answer with the song crate's rules, hands the mistakes back once, and answers the panel or the error.

## The choices

| choice | why |
|---|---|
| the conversation, the checks and the repair round run in Rust, on the route, over the rules the builder's grid uses (`apps/k7-daw-wasm/song/src/modules/ui`) | the key stays on the host and the page keeps one request and the UI; the grid reaches the same rules through the WebAssembly module, so a rule has one implementation |
| the route takes the module's request and decides the rest | what to ask is built from the request; the key, the model, the output limit and the effort are the account's and the host's, so a page cannot choose a dearer model, a longer answer or another prompt |
| the page re-checks the answer where it lands in the buffers (`useAiOrganize.ts`) | the check is cheap, and the buffers are where a wrong panel would do harm |
| the page compiles `widgets.js` with its own `new Function` and never calls it; the host reads the script's strings, comments and brackets | only a JavaScript engine reads a script for real, and the host has none: a cut-off or stray bracket is the model's to repair, any other syntax error stops the panel at the page |
| a request is checked before a minute of the account's Claude is spent on it | an update with nothing to change, a module with no name and a body that is no request are refused with a 400 |
| the model reads module and plugin names a shared library can carry, and `widgets.js` is saved, then run by the panel runtime | a hazard kept in view, not closed: the compile check runs nothing, and a library's names are as trusted as the library |
| the model is the account's (`claude-opus-5` when none is named) and `max_tokens` is 64,000 | the price of a run is the account's to set; a body's own `model` or `apiKey` is not read |
| one run answers as NDJSON: `status` lines while Claude writes, a `ping` or the last status again every 15 s, then exactly one `done` or `error` | a turn with thinking can take over a minute, and a proxy may cut a silent socket; closing the tab drops the request to Claude rather than paying for a panel nobody will see |
| the answer is cut at 8 MiB of stream | a real answer is bounded by `max_tokens`; an upstream that never stops costs one call, not the server's memory |
| a refusal, a cut-off and a second round of mistakes are the run's errors, said in one sentence | the host reads `stop_reason`; whether the text is a good panel is the check's question |

## What proves it

| proof | covers |
|---|---|
| `apps/k7-daw-backend/packages/k7-front/tests/all/ai_vectors.rs` | the sixteen recorded cases of `apps/k7-daw-backend/fixtures/organize-vectors.json`: the same model replies draw the same requests to Claude (prompt, schema, repair message), the same progress lines and the same end, byte for byte |
| `apps/k7-daw-backend/packages/k7-front/tests/all/ai_organize.rs` | the guards in their order (session, key, size, shape), a proposal, a repair, a refusal, and that only the account's key and model reach Claude, the model played by a fake |
| `apps/k7-daw-wasm/song/src/modules/ui/ai_check_tests.rs` | each rule of the check on its own widgets |
