# What the first release is

The scope k7 0.1 ships with, and what it leaves out.

## k7 runs on devices

The tray (Windows, Linux), the Android app and the iOS app (a Mac runs it as the iPad app) each carry
their own front, file service, engine registry and engine. There is no hosted instance, so none of
the hazards of a public service apply: no account is created, no storage is shared between people,
and nothing personal reaches a server. The server only serves `k7.rocks` (the root site, the
downloads page and the update feeds) and `docs.k7.rocks` (the documentation), both public, with no
login (`docs/images.md`).

## Out of scope

- **Sharing a project by link, and any k7 server to pair with.** A project moves between devices
  through git, and leaves a device as a mixdown.
- **iOS.** No CI job builds the app, and it is not distributed: it installs nowhere until signed.
- **Signed installers.** The tray's Windows build carries no certificate, so SmartScreen
  stops it on first launch; the user guide says how to open them anyway (`docs/release.md`).
