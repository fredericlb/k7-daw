# Sync, history and conflicts

Your workspaces are folders on the device. A workspace moves to another device through **git**: you
connect it to a repository on a host you pick (GitHub, GitLab, Gitea, your own), then commit, push and
pull. Nothing is sent by itself. This page explains what you see of that, what happens when two sides
collide, and how to go back in time. The steps for connecting, signing in to a host and large files
are in the in-app page "Keep a workspace in git".

## Connecting a workspace

On the home page, open the **⋮** menu of a workspace and choose **Connect to git…**, or press **Clone
from git** to bring a repository down as a new workspace. Under a connected workspace a bar shows the
remote, how many commits wait to be pulled and pushed, and how many files you changed. k7 keeps the
token or the private key in the device's keychain, never in the workspace.

Your name and email sign the commits; k7 asks for them once, and they live in **Settings → General →
Identity**.

## Commit, push, pull

| you see | what to press |
|---|---|
| files changed | **Commit**: the dialog lists them with their sizes, large files marked; write a message |
| commits to push | **Push** |
| **Pull first** | the remote has commits you lack: pull, then push |
| commits to pull | **Pull** |
| **Commit first** | you changed files since the last commit: commit them, then pull |

On an iPhone or an iPad, the workspaces live in the app's iCloud container when the build is
signed for it, and iCloud moves them between your Apple devices.

## When two sides collide

### In the text editor

If a file you are editing changes underneath you, the editor asks whether to keep your text or take the
new one. See [The text editor](./15-text-editor.md).

### Merge conflicts

A pull that cannot join the two sides opens the **merge screen**. Each conflict is a card: the key of
the song that changed, or the lines of a text, with your side and theirs. Keep **mine** or **theirs**;
**both** appears where it makes sense. When every card has a choice, press **Finish merge**; **Abort
the merge** leaves the workspace exactly as it was. While a merge waits the editors are read-only and
the music still plays.

## History

Open the **History** panel with `ui history` or its button at the right of the footer. It has two parts.

### The song's undo steps

The top of the panel lists the project's undo steps, newest first: each command or edit that changed
files. Hovering a step lights up what it touched on the graph and the timeline. `Ctrl+Z` / `Ctrl+Y`
(or `project undo` / `project redo`) walk this list. Undoing a step that bounced audio asks before
restoring the earlier file.

### The workspace's history

Below, the **Workspace history**: every edit made to the workspace on this device, with automatic
checkpoints, and the commits the repository holds. For each entry you can open it to see what changed,
file by file, **Before** and **After**, and **Restore to here**: every file of the workspace goes back to
what it was at that point. A restore is an edit like any other, so what it replaces stays in the
history and can be restored in turn. **Name this point** adds a named checkpoint (for example "before
the mix").

## Many tabs, one engine

You can open the same workspace in any number of tabs. The engine plays one project at a time: if you
open another project on the same engine, the tab that lost it says so ("Another project took this
engine" / "This project was opened elsewhere") and stops driving it. Nothing you saved is lost; open it
again from Projects to take it back.
