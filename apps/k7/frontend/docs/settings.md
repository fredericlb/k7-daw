# The settings page, and the line it draws

The user menu's Settings row, on the home page, opens `/settings`, a page of its own: `settings/SettingsPage.tsx`,
a route of the home page like `/libraries` (`appMain.tsx`), so going there reloads nothing.
The editor keeps its own settings as a panel (`components/SettingsPanel.tsx`), because those are
set while you look at the song they affect; these are not.

It holds what a **browser** may change. Everything about a machine — which audio interface the
engine opens, whether the engine is running — is set on that machine:
in the command-line engine's config file (`engine/k7-supervisor/engine.example.json`), or on the
engine page of the backend that runs it (below). Not because moving it here would be hard: those settings need a process
on that machine, and this page is served to any machine that asks.

So the split is not computer-versus-web. It is **whose disk answers**. The LANGUAGE sits at one end
of that line: no disk answers for it, and it lives in this browser alone. The separation chains
sit at the other: they are the machine's, so they are drawn on its engine page and not here.

## On the phone, the machine IS this page's

Inside the Android app (`engine/k7-runtime/docs/android.md`) the line above moves, and for the
reason it was drawn: the process on the machine is the app k7 is running in, and the phone has no
config file to hand its user. So the Engine page (below) opens on **this phone's engine** —
`settings/AndroidEngineSection.tsx`: the engine's state and Quit, full screen (Android only, a
setting of the device rather than the account), the instruments it downloads, the private CA. The Downloads section is the
computer's engine page's own (`settings/InstrumentsSection.tsx`), since what a phone keeps is chosen the same
way (`engine/k7-supervisor/docs/instruments.md`). The hint that sends a browser to the engine's machine is not drawn there, since
that machine is this one.

They are drawn only when `window.k7Android` is there (`settings/androidEngine.ts`), which the app
answers for k7's origin alone; a browser never sees them.

## Why a page and not a panel

Everything here is set once and left: the language and the AI key. Visited on
purpose and rarely — the reasoning that made the libraries a page — it does not earn a column
beside the workspaces, and a page gives it an ADDRESS. The sentence in `RepoList.tsx` that points
at the account's settings is therefore a link to `/settings`.

## Tabs, and the one in the address

`/settings` and the desktop app's engine page group their sections under tabs (`settings/SettingsTabs.tsx`):
`/settings` under General and Devices; `/settings/engine`, on the desktop app, under Engine,
Instruments, Chains, Plugins and Engines. Every section in one column runs past what one screen shows, and
the sections fall into groups nobody needs at once.

- **The tab is in the address**, as `?tab=`, so a reload or a link given to someone lands on it. It
  is replaced, not pushed: Back leaves the page rather than replaying every tab looked at.
- **Only the shown tab is mounted.** Each section fetches what it shows on mount, so a hidden tab
  costs no request; going back to one fetches it again, which is what a settings page should show.

## The language

`LanguageSection.tsx`, and it is the one row here that asks no machine anything: the language is
the BROWSER's. Paraglide keeps it under `PARAGLIDE_LOCALE` in `localStorage` — the strategy chain
this app compiles with is `["localStorage", "baseLocale"]` — so it is per browser and per machine,
travels with nothing, and survives no account. The same person on another laptop starts in English
again, which is the honest consequence of storing it where it is stored rather than a gap to fill:
what language you read in is a property of where you are sitting, not of who you are.

**English unless you say otherwise.** The shared setup
(`packages/i18n/src/vite.ts`) puts `preferredLanguage` between the stored choice and the
base locale, so a browser asking for French gets French; k7 drops that middle strategy in its own
`vite.config.ts`. The reason is that this app is written in English — the commands, the file
syntax, the module names — and half of it is deliberately not translated at all (`i18n.ts` lists
which). A French browser landing on a French shell whose every verb is English reads as a broken
translation, where an English shell with a French option on the settings page reads as a choice. It is an
app-level override and not an edit of `@studio/i18n`, which is the line that file already draws.

Each locale is named in **its own language** — English, Français — because the list is what you
read to find the one you want, and the one you are reading is the one you cannot pick: a button
that changes nothing is a button that looks broken.

The editor's own settings panel reaches the same value through `ui language <en|fr>`, and there is
no second store behind it — both doors call the runtime's `setLocale`, which writes that one key.

**One module per language in dev, one per message in a build.** The two catalogues (k7's and the
plugins') follow the Vite command (`outputFor` in `vite.config.ts`).

| choice | why |
|---|---|
| `locale-modules` for the dev server and the `paraglide-js compile` scripts | per-message modules make the dev server send every message a page names as a request of its own, nearly two thousand on a project's first load |
| `message-modules` for a build | a language module is ONE module, so the bundler puts it in one chunk shared by every page: the first screen then loads every string of the app in both languages, 300 KB of the startup path. One module per message lets each chunk carry only the words it draws |
| both write the same `src/paraglide` folders | the compiled API is the same either way (`m["key"]()`), so a build run by hand beside a running dev server leaves that server working, on per-message files, until it restarts |
The page is not itself a command: the home page's command bar carries only its own verbs, the
palette being the project shell's.

### Where the words themselves live

Every string this app draws is in `messages/en.json` and `messages/fr.json`, reached as
`m["key"]()` — the compiled functions read the locale at call time, so a module outside React
(a status line, a refusal, a formatter) calls them exactly as a component does. Three rules the
sweep that moved them settled:

- **A sentence with something CLICKABLE in it stays one message.** Cutting it in two and letting
  each half keep English word order is the usual way this goes wrong, so the message keeps NAMED
  HOLES and `src/i18nParts.tsx` fills them with nodes. Each hole carries its name through the
  string, so a language that puts them in another order still gets the right node in each.
- **A table of static labels becomes a table of FUNCTIONS** (`() => m["…"]()`), never a table
  read once at module scope: the latter freezes whatever language was loaded first. `shapes.ts`,
  `timelineTools.ts`, `rollMode.ts`, `mastering/controls.ts` and the clip view's modes all follow
  it, and their consumers call `label()` rather than reading `label`. `shellMeta.tsx`'s pane and
  panel tables use a GETTER instead, `label` still read as a field: a dozen places read it, and a
  getter is just as live.
- **`messages.test.ts` is the guard**: the two catalogues must name the same keys and ask for the
  same parameters. Nothing else in the build notices a key added to one and not the other — the
  screen just falls back to printing the key.

**A shared package under `packages/` has no catalogue and stays ENGLISH.** `ui-chains`,
`ui-chains-react`, `ui-module-builder` and `project-shell` draw
inside this app and inside others, each with its own message set; a French string in one of them
is a French string in every host. What crosses that line is a `noun`-shaped hint the host hands
in, not a sentence — or, for `ui-chains-react`, a whole `labels` object: its controls carry
sentences ("Add a module between …", "Mute Kick"), so it takes them from `DomChains`'s `labels`
prop with English defaults, and k7 fills it from this catalogue in `src/chainsLabels.ts`.

**Text a server or a shared package WRITES is sent as a code with its names, beside the English
sentence.** The screen words the code; the sentence is what a log prints and what the screen falls
back on for a code it does not know. The graph's warnings (`issues` beside `warnings`,
`src/graphWarnings.ts`) and the add-song popin's "download not set up" (`missing` beside `reason`)
are built that way. The quoted names inside a warning — a chain's, a track's, `out`, `from` — are
the file's own words and stay as written. The same rule covers text a package writes into a FILE — the
`SURFACE_MAPPING_SKELETON` notes that end up as comments in `surfaces/<id>.k7x` are English
because the file is read where the code is.

One package draws inside k7 alone and so carries a catalogue of its own, compiled by this app's
`vite.config.ts` with the same strategy chain: `k7-plugins`, the desk and the viewport
(`packages/k7-plugins/docs/i18n.md`).

## Separation chains — the engine's own setting

`ChainsSection.tsx`, the engine page's Chains tab. WHICH chain separates a record into stems, and which
one gives it without its voice (`system instrumental`). A section of the **engine's** page
and not of `/settings`, because the answer is the machine's: `stems` and `instrumental` in its
`engine-host.json`, written by that machine's supervisor (`apps/k7/backend-rs/docs/engine/chains.md`),
and read back from the machine rather than kept here — two machines of one account may answer
differently, and a saved value that only lived on the server would be a lie about what runs.

Only this machine's engine has the tab: another engine's chains are named on ITS own machine, by
whoever owns it.

**Only the library's COLD chains are offered** — a chain whose steps are all offline modules, the
only kind that can separate (`packages/song/docs/coldModules.md`); one that processes, or both
separates and processes, is not one. The file service lists them (`GET /account/cold-chains`),
against the bundled library the device reads. The instrumental picker narrows further, to the chains
whose offline step declares an `instrumental` output.

**A cold chain this machine cannot run YET is still listed**, and the reason shows under the picker
once it is chosen — almost always the model: a separation needs `htdemucs_fp16weights.onnx` (166 MB)
in `~/.k7/models`. The dropdown says which kind of "not yet" it is: « requires a download » when a
model it is missing has a `model_url` its module gives — a link under the picker then fetches it —
and « can't run here yet » when nothing here can fix it. (A chain missing both a downloadable model
and something else reads as the first: the reason under the picker still names the rest.) It was
first left out instead, and a library holding `stems` and `instrumental` then read as holding no
cold chain at all, on exactly the machine that needed telling what was missing. Choosing it is
allowed: the answer is the machine's, and it holds the day the model arrives.

Choosing writes through `PUT /account/cold-chains/stems|instrumental`, which checks the same list
again — the browser's copy is a menu, not the gate — and answers the new state, so a refused pick
snaps back. The empty option clears it: "this machine does not do that" is a real answer. A stored
chain the library no longer holds still shows, so a renamed chain does not read as "nothing is set".
`system stems` and `system instrumental` still set the same two answers from the command bar.

## AI — the device's Claude key

`AiSection.tsx`. The key and model the module builder's AI organize runs on
(`packages/ui-module-builder/docs/aiOrganize.md`). It is here, on the settings page and not in
the editor's panel, because it belongs to the DEVICE: the same key serves every project on it, and it is kept by this device's backend and
reaches no other: a second device is given its own key, never the first one's.

**The key can be set, replaced and removed, and never read.** The row shows a hint
(`sk-ant-…wxyz`), enough to tell which key is stored and not enough to use it, and the field
empties once a key is saved. A key shown again is a key that can be copied from any browser open on the device.

With no key, the builder draws no AI organize button rather than a dead one. That makes this
section the one place the feature can be found, and its row says so.

## Identity — the one name and email of this device

`git/IdentitySection.tsx`, in General. With no account to say who the person is, the device's identity
is what they typed here: the author of every git commit and the user menu's heading
(`userMenu.md`). It is stored once, in the backend's identity file
(`apps/k7/backend-rs/docs/files/git.md`), and `git/identity.ts` mirrors it for every reader, so
the commit dialog, which asks for it the first time a commit has none, and this section stay in sync.

**There is no fallback author.** An empty identity is a commit the backend refuses (`no-identity`) and the
dialog asks for, never a commit signed by nobody; the heading falls back to "Local" instead.

## Paired devices — answered here, asked on the machine

The devices allowed to drive this machine's engine over the LAN, and the requests waiting. A browser
setting only in the sense that a browser on the machine draws it: anywhere else the engine refuses,
and the section says so. `pairing.md` is the page.

## Why there are no machine settings here

Audio, the engine, its sessions and the machine itself are set where a
process on the machine can apply them: the command-line engine's config file, or the engine page of
the backend running the engine (below). A second screen for one setting is two that disagree the day one is
changed.

## The engine is a page beside it

The user menu offers **Engine** right after Settings, and it is not a tab
of this page. What this page holds is the account's; an engine is a machine's. Under one roof, a phone's engine — the reason its user opens the menu — sits behind a
tab. It stays under the `settings/` prefix, so it takes no workspace name
(`packages/web-protocol/docs/addresses.md`).

**Engine**, `/settings/engine` (`settings/EnginePage.tsx`), is the same entry everywhere, and what it
holds follows the machine. Served by the desktop app's backend — `GET /api/local` says this backend runs the
engine (`localApp.md`) — it is this machine's engine: its state, its audio interface ranking, its
plugins and its instruments, reached through k7's front, which runs the supervisor
(`apps/k7/backend-rs/docs/localEngine.md`), since the supervisor opens no socket. Inside the phone's
app it opens on the phone's engine. Anywhere else it carries the sentence saying a computer's engine is
set on that computer. No page lists or chooses among engines: the app drives the one on this device.

## About is beside it, not in it

The user menu's About row, under Settings, opens `/about` (`about.md`): the version, the licence,
the source and the third-party notices. None of it is a setting, so none of it is on this page.
