# Workspaces

A workspace is a folder on the device that holds one or more projects and what they share. It is
a git repository once you connect it to a git host, and git moves it between your devices.

Besides its projects, a workspace holds:

| Folder or file | What it is |
|---|---|
| `<name>.k7/` | A project. See [Projects](./05-projects.md). |
| The workspace's shared modules | Modules every project of the workspace can use, written `@workspace/…`. A new project's synth is one of these. |
| `_playlists/` | Set lists, which a DJ desk can name. |
| `_references/` | Finished mixes you listen against while you work. |
| `TODOS.md` | The workspace's task list. |
| `_recycle_bin/` | Where deleted projects go. Hidden from k7's views. |

The editor shows the whole workspace in its file tree, not only the open project.

## Creating a workspace

1. On the home page, click **New** at the top of the list.
2. Type a name, for example `winter`. The line under the field shows the folder that will be
   created, or why the name is refused.
3. Click **Create**, or press Enter.

The workspace is created empty, on purpose: the first project is chosen when you open it (see
below). From the command bar, the same thing is:

```text
system workspace new winter
```

### Names

A workspace name:

- cannot be empty;
- cannot contain `/` or `\`;
- cannot start with a dot.

The **New** button is greyed out while the device's file service is not answering, and while there
is no library (hover it to see why).

## The first project

A new workspace's row shows **Create the first project**. Click it:

1. The workspace opens on the page **First project of "&lt;workspace&gt;"**.
2. Pick an archetype: **arrangement**, **mix**, **video**, **live** or **live video**. The line
   under the buttons says what each one opens. See [Project archetypes](./06-archetypes.md).
3. Type the project's name. The line under the field shows the folder, with `.k7` added.
4. Click **Create**. The project is created and opens straight away.

You can also create the first project from the home page with **Nouveau projet** on the workspace's
row, or with `project new <type> <name>` in the home command bar.

## Opening a workspace

Clicking a project opens that project. A workspace can also be opened as a whole, with the address
`/@local/<workspace>` (see [Project addresses](./07-addresses.md)). k7 then opens the project that
was open last in that workspace, else the first one, else the first-project page above.

## Removing a workspace

1. Open the **⋮** menu at the end of the workspace's row.
2. Choose **Remove**.
3. Confirm. The dialog says: "The folder and everything in it are deleted, for good."

Unlike a project, a removed workspace does not go to a recycle bin.

## Where a workspace lives

A workspace is a folder on this device. Its row says whether it is connected to a git repository, and
how many commits wait to be pushed or pulled. To connect one, open the **⋮** menu and choose **Connect
to git…**; to bring one down, press **Clone from git** on the home page. The steps, the sign-in to a
host and large files are in [Sync, history and conflicts](./19-sync-and-history.md).

## Importing another DAW's project into a workspace

You can drop a project file from another DAW onto a workspace row:

1. Drag a `.dawproject` file, or an Ableton project folder zipped as `.zip`, onto the workspace's
   row. The row lights up.
2. Drop it. The dialog **Import this project** opens.
3. Type a name, or keep the file's own name (shown as the placeholder). The line under the field
   shows the project folder that will be created.
4. Click **Import**. The file is translated into a new k7 project; the file itself is not kept.
5. When it is done, the dialog lists what was read and what did not survive the translation.
   Warnings are shown in amber.
6. Click **Open it** to open the new project, or **Close**.

A lone `.als` file is refused here, because it finds its samples by their path on disk. Import it
from the editor's file tree instead, where it sits next to its samples.
