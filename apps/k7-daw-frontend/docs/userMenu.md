# The user menu

One button in every studio app that has somewhere to go: the account's heading, the app's
settings and the host's own pages. **No studio header has a settings button of its
own**: the host hands `onSettings` its own navigation, so the settings are found the same way
everywhere.

Other pages of the SAME app go in `items`, as rows under Settings: k7 on a machine that runs its own engine adds
**Engine** there (`apps/k7-daw-frontend/docs/localApp.md`). They are callbacks into the host's router.

The menu itself — the heading and the rows — is `@studio/ui/user-menu`, shared with phare
(`apps/k7-daw-frontend/packages/ui/docs/user-menu.md`). k7 is the one studio app, so the menu carries no row to another app
and no origin to resolve one against.

**No sign-out row**: on a device the local user is signed in again the moment the page reloads, so the
row would do nothing. The home sidebar passes no `onLogout`, and `@studio/ui` draws no row without one.

k7 draws it at the foot of its home sidebar, so on the home page and every page that shares that
sidebar — `/libraries`, `/tasks` and `/settings` — not on `/external`, a second device's page, and not in an opened
project: the editor's header has no room for a menu that leaves the session.

**k7's heading is the device's identity**: the name the person saved in Settings (the same one that
signs their commits), else its email, else "Local" (`apps/k7-daw-frontend/src/projects/menuAccount.ts`). There is no
account to ask, and saving the identity updates the heading at once.

Its words are k7's: `label` (the trigger's name) and `settingsLabel` come from the catalogue.
