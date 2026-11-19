# Libraries, module refs and assemblies

This page covers how a module ref resolves to a folder, where libraries are stored and versioned,
what freezes them to the app's seed, and how assemblies are recorded.

## Refs and namespaces

The vocabulary is in `packages/song-schema/src/moduleTypes.ts` (no `node:fs`, so the browser
shares it):

| Namespace | Resolves to |
|---|---|
| `@engine/<slug>` | The engine's plugin scan, `<STUDIO_HOME>/exts.json`, read by `packages/song/src/engineCatalogue.ts`. No folder; the slug is `slugifyPluginName(name)` and the type folder follows the plugin's role (`categoryForRole`). |
| `@meta/...` | k7's built-ins: modules under `packages/song/meta/` and synthesised ones (`@meta/track`, `@meta/doc`, inputs and outputs). |
| `@workspace/...` | The workspace's `_modules/` folder (`SHARED_MODULES_PATH = "../_modules"` from a project). |
| `@account/...` | The account's own library, never versioned, always writable by the account. |
| `@<library>/...` | A named library. Names follow `/^[a-z0-9][a-z0-9-]{0,39}$/` and exclude the fixed namespaces. |

`MODULE_TYPES` in the same file gives each type folder its kind and default ports; `MODULE_EMITS`
what it emits. A new type folder starts there.

### Resolution

`packages/song/src/loader.ts` builds the ref map: the built-ins first (doc modules, lanes, io,
the engine catalogue), then each module root in order: the project's `../_modules`, then
`libraryFamilyRoots("modules", dir)` (the account library first, then every library by name),
then `packages/song/meta`. A root's namespace comes from where it is, never from anything the
root declares.

`extends` is folded by `packages/song/src/moduleResolve.ts`, parent under child, deepest first,
with a cycle guard. `params` and `assets` merge per key, `engines` per engine and then per param,
`rack.symbols` and `ui.widgets` per entry; the keys in `OWN_ONLY` are never inherited. Engine and
alternative selection are applied after the merge.

`module fork` (`packages/song/src/accountFork.ts`) writes an `@account/...` module that extends
the library one and repoints the project at it.

## Storage and versions

| Concern | Where |
|---|---|
| Library home, versions, pins, the root a name resolves to | `packages/song/src/sharedFolders.ts` |
| Version manifest (`library.k7x`: `name`, `version`, `requires`, `notes`) and `LIBRARY_LEVEL` | `packages/song-schema/src/libraryVersions.ts` |
| The workspace's pins (`_library-pins.k7x`), written once | `apps/k7/backend-rs/crates/k7-file-service/src/pins.rs` |
| `library versions`, `library upgrade` | `packages/commands/src/verbs/library.ts` |
| The device's list, seeding, versions | `apps/k7/backend-rs/crates/k7-libraries/src/store.rs` |
| Routes for the list | `apps/k7/backend-rs/crates/k7-libraries/src/routes.rs` |

A library is `<home>/<name>/{1,2,...,draft}/`, the home being `$STUDIO_LIBRARIES_HOME` (set by the
file hub) or `<studioHome>/libraries`. Each version holds the families `modules`, `chains`,
`assemblies`, `presets`, `samples`, `irs`, `wavetables`, `models`.

`libraryRoot(name, from)` picks, in order: the account home for `account`; the version the reading
path is already inside; the workspace pin; the admin overlay (the draft); version 1. Reading from
inside a version first is what keeps a library's own `extends` consistent within that version.

**Why versions and pins.** A library is shared by every project of the device. Without pins, a
new library version would change the sound of every song at once. A published version is
immutable, and each workspace moves its pin when it chooses; `forksWhoseBaseMoved` reports the
forks whose original changed so the user can listen again.

### Seeding

The file hub keeps one list (`<dataRoot>/libraries.json`) and one folder per library. A listed
library with no folder is copied from `$STUDIO_LIBRARY_SEED` (`name=dir,...` or a parent folder)
under the version its `library.k7x` declares (version 1 when it has none), or created empty. A
machine that already has the library gets a seed declaring a version it lacks as one more version;
when that number is taken by another version, the seed goes in as the next free
number, its own kept as the manifest's `seed`, and is never written over anything. Nothing is fetched from an origin. The tray
sets the seed to its bundled `k7-core` (`apps/k7-tray/src-tauri/src/backend.rs`); the bundle script
`apps/k7/backend-rs/scripts/stageBundle.mjs` packs the repository's `k7-core/` (`libraryArchive.mjs`).

`k7-core/` in this repository is that seed, and its `library.k7x` is its version: a change to its
content raises it. A module and the engine code it relies on change in one commit.

## Libraries are frozen to the seed

- A device has no admin: no origin is asked for a library, and the libraries are the bundled seed's
  (`k7-core`). The Libraries page (`apps/k7/frontend/src/projects/LibrariesPage.tsx`) is read-only,
  and a library changes with an app update.
- The file service's admin overlay (`apps/k7/backend-rs/crates/k7-file-service/src/libraries.rs`) and
  the hub's admin answers stay in the code for a later rework (`docs/inventaire.md`, D2).
- Library files are addressed at the virtual path `_libraries/<name>/...` in every workspace
  (`apps/k7/backend-rs/crates/k7-file-service/src/paths.rs`). The write gate refuses everyone on a
  published version, and refuses a write to a library for lack of an admin.
- The Libraries tab of the file tree is built from `GET /module-libraries` by
  `apps/k7/frontend/src/libraryTree.ts`; the upgrade banner is
  `apps/k7/frontend/src/components/LibraryUpgrades.tsx`.

## Presets

Presets are files in a tree beside the modules: `_presets/` in the workspace, `presets/` in a
library, at `<root>/<namespace>/<module path>/<name>.k7x`. Vocabulary and schema:
`packages/song-schema/src/presets.ts`; reading and writing: `packages/song-edit/src/presets.ts`;
verbs: `packages/commands/src/verbs/preset.ts`. Applying a preset writes into the node's file and
reaches the engine as a values-only diff.

## Assemblies

| Piece | Where |
|---|---|
| Template schema (`label`, `inputs`, `outputs`, `params`) and instance record (`uses`, `members`) | `packages/song-schema/src/yaml/assemblySchema.ts`, `packages/song-schema/src/yaml/assemblyFile.ts` |
| Resolution to `GraphDoc.assemblies` | `packages/song/src/assemblies.ts` |
| `assembly new`, `delete`, `dissolve`, member renames | `packages/song/src/assemblyEdit.ts` |
| Drawing | `packages/ui-chains/src/assembly.ts` (`foldAssemblies`) |
| Template editor | `packages/ui-module-builder/src/AssemblyBuilder.tsx` |

A template is a folder under a library's `assemblies/` holding `assembly.k7x` beside an
ordinary `graph` and `nodes/`. A project instance is `assemblies/<name>.k7x` in the project.
`assembly new` copies each member to `nodes/<name>_<member>`, merges the template's wiring with the
names remapped, and refuses a clash.

The compiler never reads assemblies: they are a drawing fact, so they can never change what plays.
The reference template is `k7-core/assemblies/turntable/`.

## Tests

`npm test -w @studio/song` covers the loader, `moduleResolve`, assemblies
(`packages/song/src/turntableAssembly.test.ts`) and the library folders;
`npm test -w @studio/song-schema` covers refs and schemas (`moduleTypes.test.ts`).
