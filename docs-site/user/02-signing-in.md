# The local user

k7 has no sign-in. It runs on your own device, the [K7 DAW](./82-desktop-app.md) on a computer or the
phone app, and everything it keeps belongs to the one local user of that device: the workspaces, the
settings, the keys. Nobody is asked for a password, and nothing is sent to a server unless you
connect a workspace to a git host yourself.

## Who you are

The user menu at the foot of the home page's sidebar heads with the name you gave in **Settings →
General → Identity**, else its email, else "Local". The same name and email sign every git commit
made from the device. See [Settings](./16-settings.md).

## The language

The language switcher in **Settings** sets the interface language (English or French).

## Moving your work to another device

A workspace moves between devices through git: connect it to a repository on a host you pick, then
commit, push and pull. See [Sync, history and conflicts](./19-sync-and-history.md).

## Older addresses

k7 is not served from a site any more: the pages `app.k7.rocks`, `app.priv.k7.rocks` and the
former `st0.app` hosts do not answer. The addresses a project has are the same on the device
(see [Project addresses](./07-addresses.md)), under the device's own address.
