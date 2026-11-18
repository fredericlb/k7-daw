# `@studio/k7-wasm/song`: the compiler in the verbs' module

## Choices

| choice | why |
|---|---|
| A handle per project: `songOpen(bundle)`, drafts laid over it, `songUpdate` with the changed files, `songClose` | the compile's memo must persist between calls, and the changed list that every write already answers with says what to send |
| The calls are coarse (open, update, compile, close), never per entity | every call copies its data across the boundary; the frontend reads whole views |
| `songCompile` names the views it wants (`want`), and `errors` alone is the cheap one | a draft needs its diagnostics, not the 18 MB `web` answer |
| JSON text crosses the boundary, `web` and `graph` included | decoding the whole answer costs milliseconds; a binary layout would be a second serializer to keep in step |
| The compile runs in a Worker | a heavy project's compile would stall typing on the main thread, and the first call always pays the module's tiering |
| A keystroke never crosses: it stays JavaScript, parsed by `@studio/k7x/parse` | the parse is under a millisecond, and a crossing costs at least a copy of the file and a JSON decode |
| The module ships at `opt-level = "s"` | a third smaller to download, for a draft compile the browser never notices being slower |
| No JavaScript compile stands behind it | one Rust source answers on the server and in the browser, so two implementations never disagree |

## Alternatives refused

| alternative | why not |
|---|---|
| a `SharedArrayBuffer` answer, or a document walked from JavaScript through handles | parsing the JSON is cheap, and a shared layout is a schema of its own that needs cross-origin isolation |
| the whole project crossing on each compile | a bundle is megabytes, and the caller already knows what changed |
| compiling on every keystroke | nothing reads a compile between keystrokes |
