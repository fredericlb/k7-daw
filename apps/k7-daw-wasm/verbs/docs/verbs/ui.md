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
| a hand-named row carries a message key, never words | the words are the page's paraglide catalogue, one per language; a native interface has its own string tables, and a test resolves every key Rust can emit |
| two rows read alike when their `Text` is the same key and parameters, not the same words | Rust does not hold the words, and two keys that read alike would be a catalogue fault |
| the home page's table and its readers are Rust; what a line DOES (the file service's acts) is the page's | the acts call the page's services and hold its state, and the validation order between them is the page's flow |
| the shell's own commands keep their sentences and arguments in the TypeScript registry, handed to Rust at each build of the palette | the registry's handlers act on React state and its rows are one table with them; moving the rows into the catalogue's data is a separate change |
| a verb is a write unless it is listed as a read or as the device's, and a line naming no verb is a write | the lock fails closed, and the catalogue is the one list of verbs, so a new verb needs no second table |
| `project export` and `picture render` read their render settings with one function | the page and the server must read a line alike, and a second parser is what let them differ |
