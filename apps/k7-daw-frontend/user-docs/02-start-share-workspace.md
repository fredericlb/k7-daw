# Keep a workspace in git

A workspace can be a git repository, so a second computer, a collaborator or a backup can have it: commit your work, push it to
a git host (GitHub, GitLab, Gitea, your own server), pull what others pushed. You decide when each step happens; nothing is sent
by itself.

## What you need

- the **ssh address** of a repository, such as `git@git.example.org:me/songs.git`; k7 syncs over SSH only, so an `https://` address
  is refused at once, and there is no token, no password and no pasted key to give;
- the public key of **this device**: k7 makes it for you the first time you open the connect dialog and shows it with a **Copy**
  button. Add that line to the host, as a deploy key of the repository (with write access, to push) or among your account's SSH
  keys. The key is the same for every workspace and stays on this device (on an Apple device it is shared with your others
  through iCloud Keychain);
- a host that stores **large files** (git LFS) if the workspace holds audio: GitHub, GitLab and Gitea do. A host without it
  refuses the push with a sentence, and nothing is sent.

The private key never leaves the device and is never written in the workspace or in the repository.

The first time k7 meets an ssh server it shows the server's key fingerprint and waits: check it against what your host publishes,
then confirm. If the key of a server you trusted later changes, k7 warns you with both fingerprints and does not go on. If the
host's owner confirms the change, press **Forget the old key**: the next try shows the new fingerprint for you to trust.

## Connect a workspace

1. On the projects page, open the **⋮** menu of the workspace and choose **Connect to git…**.
2. Paste the ssh address, add the key shown to your host, then press **Connect**.

The first contact proves the address and the key. Under the workspace a bar now shows the remote, how many commits wait to be
pulled and pushed, and how many files you changed.

## Clone a repository

1. On the projects page press **Clone from git**.
2. Give the workspace a name and the ssh address.

The repository comes down as a new workspace, large files included.

## Commit, push, pull

| you see | what to press |
|---|---|
| files changed | **Commit & Push**: the dialog lists them with their sizes, large files marked; write a message, and a step list shows Committing, Pushing, Done |
| commits to push, nothing changed | **Commit & Push** pushes them |
| **Pull first** | the remote has commits you lack: your commit is kept, pull, then Commit & Push again |
| commits to pull | **Pull** |
| **Commit first** | you changed files since the last commit: commit them, then pull |

Your name and email sign the commits; k7 asks for them once, and they live in Settings, under General.

## When both sides changed the same thing

A pull that cannot join the two sides opens the **Conflict** screen. It lists the conflicts, each marked **To choose** until you pick: the key of the song that changed, or the
lines of a text, with your side and theirs. Choose **Mine** or **Theirs**, then **Next conflict**; **Both** appears where it makes sense, for an item added
under the same name on both sides (theirs is named after its author) and for a file that cannot be merged (theirs is saved
beside it). A file one side removed and the other changed is kept or removed. When every conflict has a choice press **Finish merge**.
**Abort the merge** leaves the workspace exactly as it was.

While a merge waits the editors are read-only and the music still plays.

## History

The History panel has two lists: **Edits**, what you did on this device, and **Commits**, what the repository holds.
