# The order a list is drawn in

The file service and the compile path answer every listing in Unicode code-point order
(`@studio/song-schema`'s `docs/textOrder.md`): every capitalised name before the lower-case ones,
`_` between the two, `10` before `2`. That order is the one the compiler can give identically in
node, the browser and Rust; it is not one a person reads. So every view that draws such a listing
sorts it again in the browser, with `@studio/web-protocol/tree`.

## The choices

| choice | why |
|---|---|
| one collator for every view, `@studio/web-protocol/tree`'s | the home page's tree and the file listings already sorted with it; two orders for two lists of the same studio read as a bug in one of them |
| French, base sensitivity, numeric | `Ébène` beside `Ebene`, `FM Bass` beside `fingered`, `prise 2` before `prise 10`. The interface's language does not change it: a list that reorders when the language changes looks broken |
| equal names fall back to code units (`byPeople`) | the order stays total, so a re-fetch never shuffles two rows a person cannot tell apart |
| a path is compared one segment at a time (`byPeoplePath`) | a folder's contents stay together under it; comparing whole strings would slip `live set` between `Live/A` and `Live/b` |
| sorted where the view receives the list, never in the service | the service's order feeds compiled documents and has to stay byte-identical across runtimes; the browser is the one place that knows it is drawing for a person |
| a value stays the position in the service's list | a folder-backed dropdown (`@base`, `@shader`) numbers its entries by their place in the compiled list; only the rows move, so the engine and the file read the same entry as before |

## What each view keeps

A view sorts only the names; an order it already had on purpose stays first.

| view | sorted for people | kept |
|---|---|---|
| the editor's file tree and the breadcrumb's menus | every level | folders above files, as the service groups them; pinned files on top, in the order they were pinned |
| the Libraries tab | each level's files, its chains, their nodes, its assemblies | the levels nearest first, which is precedence (`sharedModules.md`); a library's families in their fixed order |
| the module picker | namespaces, types, categories, modules | `current` first; uncategorised modules before the category submenus |
| the preset dropdown and its arrows | names within a scope | the scopes as the service orders them, workspace first; the arrows step through the same list (`src/presetOrder.ts`) |
| a base or shader dropdown, on the graph and in the builder | the names | `none` first |
| the Process menu of chains | the refs, by path | — |
| the home page's workspaces and their projects | the workspace names; the project paths | Examples last |
| the Frozen chains page | the refs, by path | the account's own above the libraries' |
| the console select, the resources, the references, the playlists, the I/O panel | the names | — |

A search keeps its own order: best match first, the collator only breaking a tie.
