# Project addresses

Every project has its own address, so a project can be bookmarked, sent to another device, or
reopened from the browser's history. The home page writes these addresses for you. This page lists
what each form means, for when you type or share one yourself.

## The forms

| Address | What it opens |
|---|---|
| `/` | The home page. |
| `/@local/<workspace>/<project>.k7` | A project in one of your workspaces. This is what a click on the home page opens. |
| `/@local/<workspace>/<folder>/<project>.k7` | A project that sits in a subfolder of its workspace. |
| `/@local/<workspace>` | The workspace as a whole: the project open last there, else the first one, else the first-project page. |
| `/@local/<workspace>/<project>.k7/@external/<surface>` | One surface of a project on its own, for a second device. |
| `/@banc/<chain>/live.k7` | The bench: a live rig seeded with a library chain. See [Opening and playing a project](./08-opening-a-project.md#the-bench). |
| `/external` | The menu of surfaces a second device can drive. |
| `/libraries`, `/tasks`, `/settings`, `/settings/engine` | Pages of the home (see [The home page](./03-home-page.md)). |

Examples, on the tray's own address (`http://127.0.0.1:18789`):

```text
http://127.0.0.1:18789/@local/band/winter.k7
http://127.0.0.1:18789/@local/band/covers/heroes.k7
http://127.0.0.1:18789/@local/band
http://127.0.0.1:18789/@local/band/set.k7/@external/desk
http://127.0.0.1:18789/@banc/@k7-core/bass/di/live.k7
```

The words `external`, `libraries`, `tasks` and `settings` are reserved: a
workspace cannot be reached by an address that starts with one of them.

## Inside a project

Once a project is open, the part of the address after the project names what is on screen:

| Suffix | Screen |
|---|---|
| (none) | The project's layout, on its first pane: the timeline when it has one. |
| `/timeline`, `/turntables`, `/mastering`, `/chains`, `/editor` | That pane, full screen. A pane the project does not declare falls back to the layout. |
| `/docs` (or `/help`) | The in-app help. |
| `/chains-builder`, `/chains-builder/<chain>` | The chain builder: the list of library chains, or one of them. |
| `?panel=event`, `graph`, `mix`, `logs` | A bottom panel open over whichever screen you are on. |

So `http://127.0.0.1:18789/@local/band/winter.k7/chains?panel=mix` opens winter on the chains pane with the
mixer panel open. The editor shell section of this guide describes the panes and panels.

## Things to know

- A project address names a folder. Renaming the project changes its address, and the old one stops
  working (see [Projects](./05-projects.md#renaming-a-project)).
- Opening a project's address claims an engine for it (see
  [Opening and playing a project](./08-opening-a-project.md)). A surface address
  (`…/@external/<surface>`) names the same project, so a tablet driving the desk and the laptop
  running the editor work on one session. The `/external` menu lists only the project an engine
  already holds, and claims nothing.
- An address of the older form `/<name>/<project>`, without `@local`, is refused with a message
  asking you to open the project from the home page. Its address there starts with `/@local/`.
