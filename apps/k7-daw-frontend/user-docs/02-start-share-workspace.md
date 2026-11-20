# Keep a workspace in git

A workspace can be a git repository, so a second computer, a collaborator or a backup can have it: commit your work, push it to
a git host (GitHub, GitLab, Gitea, your own server), pull what others pushed. You decide when each step happens; nothing is sent
by itself.

## What you need

- the address of a repository, such as `https://git.example.org/me/songs.git` or `git@git.example.org:me/songs.git`;
- a way to sign in: for an `https://` address, your user name and an **access token** (make one in your host's settings; do not use
  your password); for an `ssh` address, a key: let k7 **make a key here** and add the public line it shows to your account's SSH keys
  at the host, or **use your own key** by pasting it or picking its file (a passphrase is asked if it has one).
- a host that stores **large files** (git LFS) if the workspace holds audio: GitHub, GitLab and Gitea do. A host without it
  refuses the push with a sentence, and nothing is sent.

k7 keeps the token or the private key in this device's keychain, for your k7 account alone. It is never written in the workspace
or in the repository, and the page forgets it as soon as the server answers.

The first time k7 meets an ssh server it shows the server's key fingerprint and waits: check it against what your host publishes,
press **Trust this server**, then type your secret again. If the key of a server you trusted later changes, k7 warns you with both
fingerprints and does not go on. If the host's owner confirms the change, press **Forget the old key**: the next try shows the
new fingerprint for you to trust.

## Connect a workspace

1. On the projects page, open the **⋮** menu of the workspace and choose **Connect to git…**.
2. Paste the address, choose how to sign in, then press **Connect**.

The first contact proves the address and the sign-in. Under the workspace a bar now shows the remote, how many commits wait to be
pulled and pushed, and how many files you changed.

## Clone a repository

1. On the projects page press **Clone from git**.
2. Give the workspace a name, the address and the sign-in.

The repository comes down as a new workspace, large files included.

## Commit, push, pull

| you see | what to press |
|---|---|
| files changed | **Commit & Push**: the dialog lists them with their sizes, large files marked; write a message, and a step list shows Committing, Pushing, Done |
| commits to push, nothing changed | **Commit & Push** pushes them |
| **Pull first** | the remote has commits you lack: your commit is kept, pull, then Commit & Push again |
| commits to pull | **Pull** |
| **Commit first** | you changed files since the last commit: commit them, then pull |

Your name and email sign the commits; k7 asks for them once, and they live in the settings, under Devices.

## When both sides changed the same thing

A pull that cannot join the two sides opens the **Conflict** screen. It lists the conflicts, each marked **To choose** until you pick: the key of the song that changed, or the
lines of a text, with your side and theirs. Choose **Mine** or **Theirs**, then **Next conflict**; **Both** appears where it makes sense, for an item added
under the same name on both sides (theirs is named after its author) and for a file that cannot be merged (theirs is saved
beside it). A file one side removed and the other changed is kept or removed. When every conflict has a choice press **Finish merge**.
**Abort the merge** leaves the workspace exactly as it was.

While a merge waits the editors are read-only and the music still plays.

## History

The History panel has two lists: **Edits**, what you did on this device, and **Commits**, what the repository holds.
