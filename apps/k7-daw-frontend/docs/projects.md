# The home page — k7 opens a project, and this is where you choose it

`/` with nothing after it is a screen, not an address to correct: the account's workspaces, the
projects inside them and the libraries. It is where you
start, and where you come back to on leaving a song.

## Who draws, who holds

**k7's pages draw; the file service and the server's stores hold.** The workspaces are the
account's file service's; the libraries are bundled with the app, one list
and one folder each. Both answer on this origin, from k7's own backend. No store keeps a register of workspaces —
[apps/k7-daw-backend/docs/projects.md](../../backend/docs/projects.md) says why.

**The SOURCE of the list is the file service.** What lists the projects is the account's file
service on the server, over its workspaces (`apps/k7-daw-backend/docs/files/hub.md`). A separate index
would be a second copy of what that service already holds, and a worse one: always one change
behind the folders it describes.

Two underlying reasons, in the order they weigh:

1. **The level above a project belongs to what opens a project.** "Which workspaces does this
   person own, and what is in them" is not a catalogue question; it is the editor's first
   question, the one that has to be answered before there is anything to edit. Modules and chains
   follow the same rule: they are WRITTEN, and writing happens here.
2. **The file service is on this side.** Creating, deleting and LISTING a project all go through
   the account's file service, because a project is a folder in it. k7 is wired to that service
   for every project it opens.

The corollary is that this page holds what you WRITE: projects, modules and chains, playlists
next (`playlists.md`) — the last by a different argument, worth distinguishing: a playlist
carries no bytes, only paths of audio files that stay where they are, so the file belongs to the
workspace it was written for.

## The address follows

This screen is **the front door itself**. k7 answers at the origin root
(the device's own address, `127.0.0.1` on the desktop app).
If the first thing you want at a front door is your own work, then the page listing that work IS
the front door, and putting it behind a prefix you have to know how to type would rank it second.

**The home pages share ONE frame** (`HomeFrame.tsx`): on a tablet or a desk, a floating sidebar down the left edge with
the logo and a round command-bar button at its foot; on a phone, a floating tab bar at the foot with that button
beside it. The tabs are Projects, Surfaces, Tasks, Library (the libraries, then the frozen chains) and Settings, drawn by `@studio/ui/tab-bar`.
They are LINKS, since each page has its own address: a plain click is a router move, a modified click opens like any
link, and the current page carries `aria-current`. Settings is the last tab, lit on App settings and on the Engine page
under it; App settings is a page like the others, beside the same tabs and with no Back, since its tab is the way to it.

| choice | why |
|---|---|
| the tabs are links, not a tablist | a page is an address that copies, bookmarks and opens in a new tab |
| no mini player and no engine slot on the home | the home plays nothing; engine state reaches a person as toasts, wherever they are |
| no downloads button | a download is a task, and tasks are read in the Tasks & history panel |
| the frame holds the command bar, the question host and the toasts | a camera mode, an armed delete or a toast outlives a page change |
| a workspace's name folds its card, like the chevron; "Browse files" in the workspace menu opens its read-only tree | the name is the biggest target on the card, and the tree is a developer view that has no place behind it |
| a card whose workspace waits for a merge disables New project, Delete and the project menus and says why on the card | a write refused after a dialog was filled in loses what was typed |
| a new project opens as soon as it is made, unless it owes library modules, which the page then says | the new row would otherwise land somewhere in a long list, and the person would have to look for it |
| the list has a filter field from six projects on, matching a project's path or a workspace's name; Help is in the page's ⋮ menu and a `help` command | a long list needs a way in, and the documentation was reachable only from inside a project |
| the tabs are a sidebar wherever the window is wide enough, a bottom bar on a phone | a column of rows names every page in full and leaves the page its whole height; a phone has the width for neither |
| the logo is the sidebar's alone; a phone's page header carries none, its title (28 px) on the same row as the page's actions | a phone has no height to spare above the list, and the logo said nothing the page does not |
| the git pages (Sync settings, Clone a repository) cover the page beside the sidebar, the whole window on a phone | the tabs stay a way out of them, as on every home page |
| the page column ends 144 px above the window's foot | a phone's tab bar floats over it and must never cover the last row |

The round button opens the command bar, which also finds workspaces and projects (`workspace open`, `project open`)
and says what it did as toasts (`commandBar.md`). There is no search field in the page: the bar is the one search.

The entries are routes, so going between them reloads nothing. **Surfaces** is one of them: the surfaces other
devices on the network have open (`external.md`).

`/auth/*` follows the root: the IdP knows only one `redirect_uri`, on k7's origin, so those
routes can only be served by k7 (`apps/k7-daw-frontend/packages/web-protocol/docs/addresses.md`).

## What the address says, and what it does not

```
/                     the home page — k7 answers the origin's root
/?ws=<workspace>      …with one workspace's tree unfolded
/@local/demos/x.k7   a project, by its workspace and its folder — see below
/@banc/vocals/warm/live.k7  a bench: a live rig seeded with a library chain
/surfaces             the surfaces of the projects open on the network — external.md
/libraries            the Library page: the bundled libraries (read-only), then the frozen chains — frozenChains.md
/libraries/chains     redirects to /libraries#frozen-chains
/tasks                every workspace's TODOS.md in one list — tasks.md
/settings             what a browser may change — settings.md
```

**The path belongs to the launch.** `parseLaunch` reads it before anything is mounted, and the
FIRST segment is a sentinel — `@local`, `@banc` — so a plain word like `external` is
free for the app's own pages. The unfolded workspace travels in the **query**,
which is exactly what a SPA router owns — the argument of
[apps/k7-daw-frontend/packages/project-shell/docs/launch.md](../packages/project-shell/docs/launch.md) taken
the other way — and it bookmarks, copies, and answers the back button.

`?ws=` is set with `replace`: browsing three workspaces to see what is in them is not a navigation
you want three history entries for. What is unfolded is a folder on the machine, and its NAME
designates it: readable in the address bar, and true on the machine that
answers.

`main.tsx` reads the address once and picks a stack, rather than mounting one with pieces
switched off:

| the address | what is mounted |
|---|---|
| `/` | `AuthGate` → `Home` — the account, and nothing else |
| `/surfaces` | `AuthGate` → `SurfacesPage` — the surfaces of every project open on the network, no engine |
| `/libraries` | `AuthGate` → `LibrariesPage`, with `FrozenChainsSection` under the libraries — the account's file service, no engine |
| `/settings` | `AuthGate` → `SettingsPage` — no engine |
| `/@local/…`, `/@banc/…` | `AuthGate` → `EngineGate` → `ProjectGate` → the editor |

The `Profiler` stays on the editor's stack only: it bills every commit of the subtree to keep the rare long
ones, and this page has no bursts of commits to measure.

### The list needs the server's files, and no engine

The list of projects needs the account's file service on the server and nothing else: the workspaces are there whether or not an engine is running, so
the page does not wait on an engine probe to draw them (`settings/accountFiles.ts`'s `findAccountFiles`,
asked with `bindEngine` off). If that service does not answer, the list is replaced by the
sentence that says so, rather than by an empty screen that would look like a new account; the
header says nothing about the server, the list being where its absence shows.

**The list is asked before the page mounts.** `prefetchHome` runs as the home chunk is evaluated,
which `appMain` starts beside the sign-in check: once `/auth/me` and the hub's hello answer, the
workspace list is asked, and the page's first read takes that answer instead of asking again. Asked in
turn from the page, those are four round trips one after the other on a phone — the sign-in check, a second one, the hello,
the list — before a single row could be drawn.

What can be read and set **without an engine** is what saves installation day: the libraries here
are asked of the server. A page that needed an engine to draw would be a page where
nothing can be set until there is one.

### On a phone, the service that lists is the one still starting

The page is served before the account's file service exists: the app's backend starts k7's front,
and the front's service for that account comes up a few seconds LATER
(`apps/k7-daw-engine/k7-runtime/docs/android.md`). So the first answer on a cold start is a refusal that means
"not yet" — and reading it as "no file service" is a sentence the page contradicts the moment the
page is left and reopened.

`Home` therefore asks again — `ASK_AGAIN_MS` apart, `ASK_AGAIN_TIMES` times — and draws the list's
loader meanwhile (`RepoList`'s `lookingForFiles`) instead of the refusal. Past the last try the
refusal is the right sentence: nothing answered in fifteen seconds.

Holding the page until the backend is ready would be the other fix, and it belongs to the app
(`apps/k7-daw-android`): a page that gives up on a service a second late is wrong wherever it is
opened, so it asks again either way.


### The engine's plugin scan is a bar, never a wait

The engine scans its plugins at every start, and a project naming an external plugin waits for
that scan before it goes to the engine (`apps/k7-daw-backend/docs/file-engine.md`). The home page
says it is running — a toast at the top, "Scanning plugins" with N / M and a bar
(`projects/usePluginScanToast.ts`) — so that a project opened just then does not seem slow for no
reason. Which engine: the one the policy would play the account's projects on, else the one the
account remembers, else the only one that answered; under the desktop app that is the machine's own. With
none, or one that reports no scan, there is no toast. It is asked after the list, beside it, and
never in front of it, for the reason above: the list needs no engine. The page listens on a
socket that claims nothing (`apps/k7-daw-frontend/packages/engine-browser/docs/access.md`), since it plays nothing.

## The rule that holds everything

**The folder's suffix makes it a project.** `mysong.k7` opens in k7, and
`apps/k7-daw-frontend/packages/web-protocol/projects.ts` is the whole rule. A folder with no suffix is not a project,
even if it contains a `project.k7x`.

This page keeps `launchUrl` on each row rather than building its own addresses: the address is a
contract with the gate that reads it back, and one builder is one encoding.

Everything an k7 project can BE is in the **surfaces** it declares inside the `.k7`, rather than
in some kind of folder: a `viewport` and video lanes for picture
(`apps/k7-daw-frontend/packages/song-video/docs/picture.md`), a `turntables` surface and the playlist it names for a set
(`apps/k7-daw-frontend/packages/song/docs/setProject.md`). That is why the three archetypes in the creation menu are
named after their surface.

## This screen's command bar

At the bottom, full width, exactly as in the editor and for the same reason: it is where you type,
so it is always there, never summoned. `apps/k7-daw-frontend/docs/commandBar.md` describes the component; what
follows is what sets it apart here.

**Its vocabulary is its own** (`src/projects/homeCommands.ts`), and not `LOCAL_COMMANDS`. This
screen has no open project, no engine session and no file: the editor's catalogue here would be a
list of things you cannot do, and the half that comes from the file service would not even answer
— `/complete` and `/command` want an active project. The verbs are the page's buttons, and one screen of its own:

| the line | what it does, and through what |
|---|---|
| `project new <type> <name>` | the archetypes, then `projectFolderName` adds the `.k7` — scaffold on the server |
| `project copy <name> <workspace> [new name]` | copies it into that workspace — the name is optional, and is the source's without it |
| `project rename <name> <new name>` | renames the folder in place, suffix kept |
| `project delete init <name>` | **names** the project and says what confirming will do. Writes nothing |
| `project delete confirm <name>` | deletes that one, if it is the one that was named |
| `system workspace new <name>` | an empty workspace on the server |

**`camera mode` is hidden.** Its verb and screen (`src/camera/CameraMode.tsx`) are kept, and
`CAMERA_MODE_LISTED` in `homeCommands.ts` shows it again once cameras return as an app feature. What it
is and how the picture travels: `apps/k7-daw-frontend/packages/web-protocol/docs/cameras.md`.

**Deleting is two verbs, and that is the confirmation.** A palette can fire a destructive line on
an Enter landing on a fuzzy match; the bar has no dialog to ask a second question with, so it asks
a second verb. Naming the project twice is what makes the second keystroke a decision, and `init`
is the only place you READ what deletion does — the folder moved to the workspace's recycle bin.
The confirm demands the same name, or the second line would be
a formality. The arming is **consumed by any command**, not only by its confirm — the rule the
graph's pending insertion already follows (`docs/shell.md`) — so a destructive verb cannot stay
armed behind three unrelated lines.

**Which workspace a line lands in.** The `project …` verbs name no workspace: it is the one the
address unfolded (`?ws=`), or failing that the only one there is. Between several, the line is
**refused by naming them** rather than choosing one — creating a project in the wrong workspace
is a change made where nobody was looking.

**Buttons and bar go through the same functions** — `createProject`, `copyProject`,
`renameProject`, `deleteProject`, `createWorkspace` in `workspaceOps.ts`: one implementation, two doors, as the
file tree's menu and the `fs …` verbs already have (`docs/shell.md`). What the bar adds is doing
it without the mouse, and saying so in a status line above it: refusals in amber, because a silent "that did nothing" is
avoided everywhere else. A line this
screen does not know is refused **here**: there is no service behind to pass it to, unlike the
editor's bar, whose lines end at `/command`.

## The LIBRARIES page: the libraries read-only, the workspaces' pins with Upgrade

The libraries are named folders of the modules and chains every project inherits, **bundled with the
app**: a library changes with an app update and nothing on this screen edits one. It is a PAGE,
`/libraries` (`LibrariesPage.tsx`, drawing `LibrariesCard.tsx`), which the **Libraries** tab
links to — reserved like `/surfaces`, and read before `parseLaunch`. It lists each library
by name with the versions it holds, says so plainly, and flags a library whose folder is missing or
whose version needs a newer k7 level. A device user is never an admin, and the backend answers 403 to
a write or a publish anyway ([apps/k7-daw-backend/docs/files/hub.md](../../k7-daw-backend/docs/files/hub.md)).

Under the libraries, **each workspace's pins** (`LibraryPins.tsx`): the versions it pins, the newest this k7 runs, and
an **Upgrade** when the pin is behind. Upgrade rewrites `_library-pins.k7x` and nothing else — the home has no
engine session to run the editor's `library upgrade` verb — so the forks whose base changed are listed by the verb,
not here, and the songs recompile when next opened (`docs/library-versions-plan.md`).

Without a library k7 refuses to open anything — not on principle, but because no `@k7-core/…`
reference would resolve, and a song opened with all its shared modules missing reads as a broken
song rather than as an incomplete install. The gate then shows the same card, saying the libraries
come with the app.

**A list of NAMES, one folder each.** The name is the level its refs use — `@drums/audio/kick` — and
the first library is called `k7`, which is what every `@k7-core/…` means
([apps/k7-daw-frontend/packages/song/docs/sharedFolders.md](../packages/song/docs/sharedFolders.md)). A row whose
folder is not there says *not on the server yet*; the bundled seed is the only source of a library's
content ([sharedModules.md](sharedModules.md)).

**The home page says something only when something is wrong** (`LibrariesNotice.tsx`): no library, or
a listed one with no folder. It shows one amber line at the top, with a link to the Libraries page.
A healthy list costs the page nothing.

### The debt is said at creation, not discovered at opening

The card checks the DECLARATION of the library, never its contents, so a `mix` scaffolded on a
new account would otherwise open with **all** its shared modules missing — exactly the "broken
song rather than unfinished account" reading the card exists to prevent.

`createProjectIn` therefore answers a `missing[]` beside the `project`: the `@k7-core/…` refs the
scaffold has just written and the workspace does not resolve
(the scaffold's needs, `apps/k7-daw-backend/packages/k7-file-service/src/edits/project_create.rs`). Three things worth noting:

- **The list is read out of the text the scaffold writes**, not kept beside it. An archetype whose
  nodes change module cannot make the list lie.
- **Only `@k7-core/` counts.** `@workspace/` is the workspace's `_modules`, `@engine/` the plugin scan
  and `@meta/` the built-in modules: none of those three waits on a library. That is why
  `arrangement` and `video` owe nothing and `mix` is the only one of the three that opens broken
  on a new account. The asymmetry is pinned by a test.
- **The sentence promises no catalogue.** It names the modules owed and says you write them
  yourself, so as not to suggest something is going to fetch them.

### No workspace is made before a library is there

**New workspace** is greyed until a library is listed — and the page's **New project** with
it, there being nowhere yet to put one — saying so on the button rather than on the
click, and the file service refuses the same call on its own side. A workspace made before the
library exists is a folder whose every `@k7-core/…` resolves to nothing — the same half-set-up account
the libraries notice exists to prevent, one level down and harder to read, since by then there is a
workspace and a project in it to blame instead.

## Dropping a foreign project on a workspace

A `.dawproject` dragged onto a workspace's card is imported into
it. The card lights up while the drag is over it — one workspace at a time, so what is about to
happen is never ambiguous — and the drop
opens a popin that asks **what to call it** before anything is uploaded. It defaults to the file's
own name, shows the folder that will be created under it (the same `projectFolderName` rule
`Nouveau projet` uses, suffix included), and refuses the names that dialog refuses.

The page's verbs Worker translates the file (`ProjectsPane`'s drop starts one for the row's workspace, since this
page has no open project and so no file API session to share), uploads its samples into the new folder and lands the
texts with one change set. **The dropped file is not kept**: a `.dawproject` is a zip carrying its own audio, and keeping
it beside the project it produced would put every sample in the workspace twice.

What comes back is the **import log** — what was read, and every warning the mapper raised — shown
in the popin before anything opens, because a translation is lossy and finding that out inside the
song is finding it out too late (`apps/k7-daw-frontend/packages/translator/docs/import.md`). Then a button opens the
project, in this tab like every other project link here.

Only `.dawproject` and a zipped Ableton project are taken, and the importer refuses anything else on its own side. A `.als`
resolves its samples against its own folder, so one dropped alone would import a silent project;
it is imported from the file tree of an open workspace instead.

The row lights up for **any** file drag, because a `dragover` carries each item's kind and MIME
type and never its name — and a `.dawproject` has no MIME type. So the judgement happens on the
DROP, where the name is finally readable, and a file this cannot take is refused in a sentence
rather than by a highlight that never appeared.

## The BENCH, a rig for one chain

A chain is something you HEAR: what it does cannot be read in its file, and its parameters only
exist inside a project with the plugin loaded. The bench is that project — a live rig written again
on every open and saved nowhere ([apps/k7-daw-backend/docs/banc.md](../../backend/docs/banc.md)).

**This page has no bench card.** The bench is reached from the Frozen chains section of the Library page
(`frozenChains.md`), where it builds and edits the account's chains. `/@banc/<chain>/live.k7` seeds the rig with a copy of a library
chain, and the bare `/@banc/live.k7` is the rig alone. It edits no bundled library, so a bench that names a bare library is refused.

**The bench is in no list of workspaces.** Its `Banc` folder is a workspace on disk, but it is
reached by an address and rebuilt on every open; a row for it on this page would offer to open
or delete a rig that is thrown away anyway. `workspaceNames` drops it with the hidden ones.

## A project opens in THIS tab, and the project's name returns

Every departure — the indented row under the workspace, the tree's button, the empty workspace's
link, the import's **Open** — opens in the same tab. Opening a project is the main reason to be on
this page, and a new tab for each one stacks tabs nobody closes, each one an editor holding a
session. The way back is the Back button of the project's header and the project's name, which links here too (`projectsUrl`, the same address both gates use when they send
you back). The browser's own Back works as well: each crossing is a history entry.
Leaving with an edit the autosave is still holding back asks first (`hasUnsavedWork`,
`apps/k7-daw-frontend/packages/ui-editor/src/unsaved.ts`), whichever way you leave.

None of it is a page load, and none of it is a `<Link>` either: the project is the router's
`basename`, so a router cannot route outside it — the crossing is `appNav`'s, which swaps the router
for the next scope's (`shell.md`, `apps/k7-daw-frontend/packages/project-shell/docs/launch.md`).

`RepoList` and `RepoTree` receive a `projectHref`, not an `onOpenProject`. **An address and not an
action** — the page computes where the row leads, the view makes it a link. That is what keeps
ctrl-click (a new tab, for whoever wants one), shift-click and "copy link address" working: the
browser decides where a click lands, and a plain click lands here. A navigation in an `onClick`
would answer the same thing to all of them.

## The tree is not the normal path

What you come here for is a project, not a workspace. `GET /workspaces/projects` returns, per
workspace, the list of its projects, and the screen draws them **indented under their own**, each
leading straight to its application. There is no `projectCount` field beside it: it is
`projects.length`, and two sources for one number always diverge.

There is no `null` in that response, and that is the clearest gain: "not known yet" would be a
third state to tell apart from "no projects", and a workspace shown without projects when it has
not been read yet is a lie that corrects itself ten seconds later, so one nobody reports.

Projects are found **at any depth**, `songs/hiver.k7` included, and a project inside a project is
descended into and reported — a `.k7` filed inside a `.k7` folder is a real arrangement. That is
`projectDirsUnder` in the file service, the same walker as the list the editor already polls. The
tree keeps its button: it is for looking at what is inside, including folders that are not
projects.

### One card per WORKSPACE

The list shows the account's folders under its `workspace/` on the server, every one addressed the
same way, `@local`. `Examples`, seeded with the demos on first contact, is what a new account has
on the day it has nothing else.

Each workspace is a card (`ProjectsTable.tsx`), a `region "Workspace <name>"`: a header with its name, its
count, its git pills (`git/GitBar.tsx`), New project and ⋮, then its projects as rows (`ProjectRow.tsx`).

| choice | why |
|---|---|
| a card per workspace, not a table | a workspace has acts of its own (Pull, Save, New project) that a band squeezed into a column |
| a card folds | the list grows by workspace; folding one keeps the others in view |
| no virtual scroll | a card's rows skip rendering while off screen (`content-visibility`), which costs less than measuring them |
| the card carries `data-workspace` and the drop handlers | a drop, and a test, find a workspace by its card |
| Pull runs in place, with a busy pill and no percentage | a pull reports none; a bar that moved would lie |
| Save is one act, run in place with its steps as toasts (`git/saveAct.ts`) | Push alone is retired; the pill stays enabled when only a push is owed |
| the pills open the Conflict screen through `gitScreens.tsx` and Sync settings as a home page (`gitPages.ts`) | one screen host per page, never a dialog per card |
| "On your other devices" follows the last card | it offers clones, which are rarer than the work above it |

## `@local` — every project's address

```
/@local/Examples/synth-demo.k7     a project in the Examples workspace
/@local/hiver.k7                   a workspace that IS the project
```

`@` cannot start a workspace's folder name, so the sentinel can never collide with a workspace,
and the gate opens the folder that is already there: nothing to look up, nothing to download.

**`parseLaunch` has nothing special to do.** `/@local/Examples/synth-demo.k7` reads as repository
slot `@local`, project `Examples/synth-demo.k7` — the project part stops at the last segment claiming
the app's suffix, the ordinary rule. `localTarget` detaches the workspace from the front, and that
is the only rule specific to `@local`. A workspace that IS a project has nothing after its name
and the project is `"."`, so nothing downstream changes.

**It is the ONLY address.** No register maps a short name to anything else, so `projectHref`,
`workspaceHref` and `projectHref` take no set of registered names and choose nothing: a project
is a folder in the account's workspaces, and its folder name is what addresses it.

What that gives up is portability between accounts: a link pasted to somebody names a folder in
yours. What it buys is one answer instead of two, and no service to ask. It also makes the
second-device case free rather than delicate — `projectClaimKey` is computed from the address, and
where there is one address both ends compute it alike, which is the failure
[external.md](external.md) spends a section warning about.

**It is also the starting point.** A workspace is created HERE, as a plain folder on this device
(`apps/k7-daw-backend/docs/file-workspaces.md`); its address does not
change, and it reaches another device only through git or by being downloaded.

**`@local` does NOT require a library.** The bench raises `no-libraries` first,
because an unresolved `@k7-core/…` reference makes a rig read as broken rather than as an unfinished
account. But a demo on this machine needs none, and no library is what a NEW account has: demanding
one would put the demos behind the one thing they exist to precede. A reference that does not
resolve already reports itself, on its node.

### How the tree opens

The **first level is unfolded** as soon as the machine answers, and nothing below: a workspace that
opens fully folded hides behind a click the only thing you came for, and unfolding everything would
drown a project in the neighbour's `node_modules`. That is done **once per workspace**, not on
every tree received: after that the folds belong to the person.

Rows are sorted **alphabetically, at every level**, folders and files mixed. The comparator is
`@studio/web-protocol/tree` — `Intl.Collator`: case and accents do not count, and `prise 2`
precedes `prise 10`. It is in a package and not in an app because every tree of the studio sorts with the same one,
and two orders for two trees of the same studio read as a bug in one.
Every other list the file service answers is sorted with it too (`listOrder.md`).

The tree comes from `GET /workspace-tree` — the home page's own route, asking it of a workspace it
has not opened. It is `FileNode`, the same type as everywhere else, with the same
`project?: boolean` flag on folders and `app` saying which application opens it. The drawing knows
only that type, never the service that answered.

### A workspace's files

A file in the tree opens **read-only** beside it (`src/projects/FileViewer.tsx`). This screen
writes nothing: what changes a project is the editor, where autosave and the compiler's checks
hold, and a second place to type into would be a second set of rules.

The viewer draws the file's content as it is on the server, with line numbers, and a binary file
as a sentence. The tree is read once per open: it is a look inside, not a monitor.

## Getting a workspace: one gesture

**New workspace** asks the file service for `POST /workspaces`, which creates the folder. Nothing to
configure — no key, no remote, no host. A workspace is local to the device that made it, and it is
connected to a git remote when its owner chooses to (`git.md`).

**And it is EMPTY.** No default project: the archetype decides what the screen shows — a timeline,
two decks, a rig with no surface — and that is a question badly answered from a popin on a list,
before having seen anything. The row of an empty workspace has one link, "Créer le premier
projet", which opens the workspace; the editor puts the assistant there (`FirstProject.tsx`). It
is also the only way in while the workspace is empty, since rows link PROJECTS and there are none:
`workspaceHref` builds the workspace's own address — `/@local/hiver` — and the project there is
`.`, which the machine resolves
(`apps/k7-daw-backend/docs/file-workspaces.md`).

**A workspace's row carries a ⋮** (`WorkspaceMenu.tsx`) holding the one act on the whole
workspace: **Delete**, in red. It sits behind a button for the reason a project's acts do: it is
rare, and a button beside the row's own link would compete with it. It goes through `askConfirm`,
which says what is lost: the folder and everything in it, for good — there is no bin a whole
workspace fits in.

Clicking a project goes straight into its application, in this tab, at
`/@local/<workspace>/<dossier>` — an ADDRESS, naming the folder on the machine, which the gate
opens without asking anybody ([launch.md](../packages/project-shell/docs/launch.md)). That
gate is `@studio/project-shell`, and an app outside this repository that mounts it comes back
here too on leaving a project: a second picker on its side would be the second thing to keep in
step that this package exists to avoid.

## Creating, copying and deleting a project — on the machine, not here

The page's primary button is **New project** (the archetype, a name, and which workspace when
there are several), each workspace's block carries one of its own that already knows where, and
each project a **⋮** holding the three acts on one: copy, rename, delete. Neither is done by a store of the server's: what this page
calls is the file service the whole app uses as soon as a project is open, not a seam that would
serve only here.

**The three sit behind one button** (`ProjectMenu.tsx`) because the row is first of all a LINK —
clicking it opens the project, which is what somebody came for — and three text buttons beside
that link compete with it while being wanted far less often.

The dialogs are mounted **outside** the menu, by the menu's own component, with the menu holding
only which of the three is asking. A Radix menu unmounts its content on select, and a dialog
inside one goes down with it the instant it is chosen.

They are **explicit calls**, with `fileBase` and the token on each one (`projects/api.ts`), and not
the editor's global `configureFileApi`. That one is set ONCE, on the workspace of the single
project a tab has opened; this page touches a different workspace on every row.

| piece | where |
|---|---|
| scaffold at default settings | `POST <fileBase>/project/create` → `createProjectIn` |
| deletion (the folder goes to the recycle bin) | `POST <fileBase>/project/delete` |
| a copy into another workspace | `POST <fileBase>/workspaces/copy-project` |
| a rename in place | `POST <fileBase>/project/rename` |

Each is ONE call and nothing after it: the write lands in the workspace on this device, and git carries
it on when the owner commits (`git.md`). There is no second step to fail after the first has
succeeded, so the dialogs report one outcome.

### What a row says, and what it leaves out

A project row opens it, its name being the link, and what it shows is what somebody scans a list of
songs BY: the archetype's glyph in place of a bullet, the name, a violet dot on the workspace's
active project, its `project.tags` (set in the Tasks panel, `tasks.md`), its folder when it sits in
one, and how long ago it changed.
The archetype is read back from the surfaces the project declares (`projectKind.ts`), the same
question the editor answers to choose its panes, so a project whose surfaces were edited by hand is
drawn as what it now is. All of it comes from the listing the page already polls
(`apps/k7-daw-backend/docs/file-workspaces.md`), whose 5-second window is also why a tag added in the
editor can take up to five seconds to appear here.

There is **no app badge**: every project this page lists is opened by k7, and a label that is the
same on every row says nothing five times. A workspace's project count is plain muted text and not a
violet pill, for the reason the next section gives.

### Every entry has a glyph of its own

On the icon rail the glyph is all an entry has, so no two entries share one.

## An empty page offers the act that fills it

Every list on this side that can be empty draws `@studio/ui/empty-state`: a glyph, a title, one sentence saying what
would be here in the reader's words, and the button that makes the first one — New workspace on Projects. The button
is there even when the New menu has the same one: an empty page is read from its middle. A page with nothing to do
from here, Tasks, draws the block without one.

## The page's hierarchy: one title, one menu for New, violet for state

The top bar (`PageFrame.tsx`) is a large title with a count line under it, and at its right **New** (a menu: New
workspace, Clone from git), **More** (Show examples). A project is made from its
workspace's card, where its destination is already known, so the page has no New project button. The list is capped
at `max-w-5xl`.

**Every page of the home side shares that frame**: Projects, Tasks and Library sit in the same
column under the same top bar, a page's own controls in it. A page of narrow rows (settings, libraries) keeps them
narrow inside the frame so moving between pages never moves where the title starts.

**The chrome is neutral; violet is a state.** The accent is kept for what is lit, selected or the main action — the
current tab, the archetype picked, the primary button.

### The suffix decides, and the name is shown first

`projectName.ts` is pure and tested: it adds the suffix, removes an already-typed suffix —
"hiver.k7" stays "hiver.k7" rather than becoming "hiver.k7.k7" — and refuses
separators rather than replacing them silently. A suffix is read **only at the end**: "prise v1"
is an ordinary folder name and nothing touches it. That is already `projectApp`'s rule, and
`projectName.test.ts` pins it on both sides. The resulting folder is shown under the field
**before** confirming: here the name decides everything.

### The three archetypes, named after their surface

| archetype | surfaces | what it writes |
|---|---|---|
| `arrangement` | `timeline` | one track, one synth, one pattern |
| `mix` | `turntables` | two decks, a crossfade, and a `playlist:` to fill in |
| `video` | `timeline` + `viewport` | the arrangement, plus a picture lane |

**On screen they are named for what someone wants to make**, not for the surface: Song, DJ set,
Video, Live input, Live video, each with a sentence in plain words (`shape.*` messages). "mix" read
as mixing a song, and "graph", "surface" and "rig" describe the app rather than the music. The ids
stay the archetypes', since the command bar (`project new <type>`) and the file service speak them.

Everything is a `.k7` folder opened by the same k7, so "what kind of project" has no answers to
tell apart — what differs is the surface the project DECLARES
(`apps/k7-daw-frontend/packages/song/docs/surfaces.md`). There is no "the graph alone, no surface" archetype: an entry
whose whole description would be "none of the three" is not a choice made on purpose, and a
hand-written rig opens anyway. The `live` MODE is something else: it is what `mix` declares.

`createProjectIn` tells them apart in a `Record<ProjectShape, …>` and not in a chain of `if`s: a
fourth archetype added to `PROJECT_SHAPES` will not compile until somebody says what it writes.

**The picker draws them, all at once** (`ShapePicker.tsx`, `shapeSchematics.tsx`). Each archetype is
a card with a line drawing of the screen it opens on — lanes and a playhead, two platters and a
crossfader, a frame over lanes, an input through a fader to a speaker, a webcam into a frame — and
its description, every card's visible and not only the picked one's. The archetype decides what the
whole project BECOMES, and five text buttons with one line under the chosen one asked for that
decision by name alone. The cards are native radios under the drawing, so the arrows, the single
tab stop and what a screen reader announces are the browser's; the drawings are pictures of a
screen rather than icons, hence drawn here and not taken from the icon set. The dialog widens for
them (`wide` on `NameFormDialog`), and so does the first-project page.

> **The file service knows these names, and a machine behind says so.** `shape` travels to
> `POST /project/create`, whose `z.enum` is `PROJECT_SHAPES`. A machine whose service is older
> answers 400 — a failure that is visible and named, which beats a template guessed alongside.

### Copying a project

"Copier" takes a project into another workspace on the same machine: pick where it lands, name the
copy, and the original is not touched (`CopyProject.tsx` → `workspaceOps.ts`'s `copyProject`).

The destination list leaves out two workspaces. Its own, since a copy needs somewhere else to be
useful and the same-workspace duplicate is a different gesture; and any workspace that IS a single
project, where nothing under the root is listed and the copy would exist without ever showing up.

### Deleting a workspace asks for its name

A project deleted goes to a bin; a workspace deleted is gone, folder and all. So the row's ⋮ says
"Delete workspace…" rather than "Remove", the confirmation counts the projects it takes with it, and
it asks for the workspace's NAME typed in (`askText`, `WorkspaceMenu.tsx`): a click is what a habit
gives, and this is the one act on the page that cannot be undone.

### Demos

The `Examples` workspace holds the bundled demos, which the file service mirrors on every start —
an edit to one is overwritten, a deleted one comes back (`apps/k7-daw-backend/packages/k7-file-service/src/workspaces/demos.rs`).
Mirroring is what keeps them working across versions, so it stays; what changes is that it is SAID.
The listing marks a managed demo `demo: true`, and its row wears an amber "demo — resets" badge; the
workspace poll says `activeIsDemo` for the open project, and the editor draws a line under its
header with "Copy to my workspace" (`components/DemoBanner.tsx`), which opens the same dialog as
"Copier" and goes straight into the copy. Only a bundled folder at the top of `Examples` counts:
anything else put there is the person's own and is never touched.

`Examples` takes **no new project** — not created, imported, dropped or copied into: the file
service refuses all four by name (`refuseInExamples`, `apps/k7-daw-backend/packages/k7-file-service/src/edits/project_create.rs`),
and the home page offers none of them on its row. A demo is copied OUT to be worked on, and a
person's project sitting among folders the bundle rewrites on every start reads as one of them.
The workspace comes **last**, after the person's own, and is **not listed** once there is a
workspace of one's own: the demos are then the reference rather than the work. **Show examples**,
in the ⋮ beside the page's heading (`ProjectsMenu.tsx`), brings it back. Until that toggle is used
it is on only while Examples is the only workspace, since a new account's page would otherwise show
nothing at all; once used, the browser keeps the choice. While Examples is all there is it cannot be hidden (the
item is disabled and says why), and the page's empty state, with New workspace and Clone from git, stands above its card.

### Renaming a project

`project rename <project> <new name>`, and the ⋮'s "Rename". The folder moves on the machine, in
the workspace it is already in (`RenameProject.tsx` → `workspaceOps.ts`'s `renameProject`).

Three things it does not do, each for its own reason:

- **It does not change the SUFFIX.** The suffix is what says which app opens the folder
  (`projectApp`), so renaming a `.k7` into anything else would stop it being a project.
  `projectFolderName` puts the source's own back whatever is typed, and the file service refuses
  the mismatch by name rather than trusting the caller.
- **It STOPS the engine when that machine is playing the project**, exactly as a deletion does,
  rather than refusing with "stop it first". That is why it is not `renameEntry`, whose
  `guardMutable` refuses the active project outright: a rename needs the handles released for the
  same reason a delete does, and asking somebody to go and stop the transport in another tab
  before they may rename a folder is a refusal that teaches nothing. The dialog says it before the
  click. What the project's marker does after is the one place the two acts differ — below.
- **It does not chase the address.** A link to the old one stops answering, and the dialog says so
  before the click rather than after. That is the honest cost of a name being an address
  (`apps/k7-daw-frontend/packages/project-shell/docs/launch.md`), and the alternative — a redirect table on the machine
  — would be a second place where a project's name lives.

**The active project FOLLOWS the rename.** A delete clears the marker, because the project has
ended; a rename leaves the same project under another folder, so the active-project marker is re-pointed at
it and reopening the workspace lands where it was. The engine is stopped either way, and starting
it again is opening the project — the gesture that starts one anywhere else.

## The project list is polled

The list has no token in front of it — no `/revision` to ask first. The service answers the
list itself, behind a five-second window on its side (`LISTING_RESCAN_MS`, the same as the one
under `workspaceInfo`'s project scan: without a floor, every tab on the home page does a `readdir`
per folder every three seconds). Asking first whether anything moved would be one more round trip
to save an already-memoised scan.

A list five seconds behind is a row that has not appeared yet, i.e. a wait. The compiled document
cache refuses that trade and has to — a stale compile is confidently wrong about a song — and that
is the difference worth noting. A workspace made here empties the window immediately
(`forgetListing`), so the only possible lag comes from a hand outside the process.

`onChanged` re-reads the list at once, after a write just made, rather than waiting for the next tick.

## What to check in a browser

The suites here run in node and mount nothing: `projectName`
and the bar's verbs (`homeCommands.test.ts`) are covered, the screen is not.

- open `/` while logged out → the login screen, then the home page;
- **with no engine machine**: the project list is replaced by the sentence that says why, the add
  field and **New project** are greyed, and the libraries can still be read;
- **with a machine and NO library**: `Examples` is there, and one of its demos opens
  at `/@local/Examples/<démo>.k7` — the installation-day check. **New workspace** is greyed, and
  hovering it says a library is needed first;
- **the libraries page**: it lists `@k7-core` with its versions and says a library changes with an app
  update; there is no add, no remove and no publish;
- **delete a listed library's folder**, reload the home page → the amber line says the app holds no
  folder for some libraries, and the row reads *not on the server yet*;
- **open a chain's bench, then go home** → `Banc` is not in the list of workspaces;
- **drag a `.dawproject` onto a workspace card**: that card alone lights up, the popin opens on the
  file's name, and the folder under the field follows what you type. Import → the log, with its
  warnings in amber, then **L'ouvrir** opens the project in this tab, and Back shows the card's
  project count already gone up. Drop the same file twice under the same name → refused by name, and the
  first import is untouched. Drag a `.wav` onto a row → it lights up like any file drag (a
  dragover cannot read a name) and the drop is refused in a sentence saying what it takes;
- unfold a workspace, reload → the same one unfolded, `?ws=` in the address;
- create a project → the row appears **with no refresh**, in five seconds at worst. Longer means
  the list is not coming from the machine;
- open a project → it opens in THIS tab; the project's name comes back here, and ctrl-click still
  opens a new tab;
- leave a project from the editor, and lose its engine to somebody else → both come back here;
- delete a project → it leaves the list, and its folder is in the workspace's `_recycle_bin/`;
- the end-of-shares line shows under the library notice, and **Dismiss** hides it for good in this browser;
- **the bar**: hidden until Ctrl+Shift+P, double-Shift or the Commands icon at the bar's left edge summons it, on every home page; `project new` opens a three-value field then asks
  for the name; `project delete init` then `confirm` deletes, and `confirm` alone refuses; an
  unknown line is refused without going anywhere.
