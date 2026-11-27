# The command bar's, the menus' and the edits' rules (`verbs::ui`)

What the page asks of the `verbs` crate besides running a line: how the bar ranks and fills, how a
right-click is derived, what a sent line is expected to do. The page reaches it through `k7-wasm`'s
`ui.*` ops (`apps/k7-daw-frontend/packages/k7-wasm/src/ui.ts`).

## Choices

| choice | why |
|---|---|
| the rules sit in the `verbs` crate, and the browser's module is built with its `verbs` feature | the catalogue and the rules over it are one crate; the linker drops what no op reaches, so the module carries the catalogue and these rules and no verb |
| every input arrives in the request: the catalogue, the prefs, the lines, the lists | a rule that read a project would need a file system, which the module has not; a native interface passes the same JSON |
| a ranking answers indices into the list it was asked about, never the rows | the page keeps its own row types (labels, translations, React keys) and Rust never has to know them |
| one sync JSON call per rule, with no handle held between calls | a keystroke costs about 0.1 ms over the whole catalogue (measured by `ui.test.ts`); the bar needs no cache |
| the matcher is a port of the 24 lines of `fuzzy.ts`, not a library | a subsequence score over a short list needs no dependency (decision 10) |
| ties are broken by `order::locale_cmp`, the root collation order of ASCII and Latin-1 names | `localeCompare` is the page's tie-break and the catalogue's words are ASCII; a full ICU collation would outweigh the rule |
| a prefs entry is a JSON object and only the keys named here are read or written | the preferences document is the account's, and a key an older page wrote must survive a newer one's edit |
| a field set to `null` in a patch is taken back | `undefined` does not survive JSON, and "taken back" is what it meant |
