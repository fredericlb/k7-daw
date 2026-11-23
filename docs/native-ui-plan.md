# The native UI plan: SwiftUI for the home side on Apple platforms

The pages reachable from k7 DAW's home get a native UI, Apple platforms first. Opening a project still shows the web
editor. This page holds the choices; the code of each phase points here.

## The decision

| choice | why |
|---|---|
| the HOME side is native: the Projects home (workspaces and project rows), Tasks, Libraries with frozen chains, Surfaces, Settings (General, Devices, Services, Downloads, About), the git pages (Sync settings, Clone), the Forgejo and GitHub account flows, "On your other devices", the first-project wizard, the Engine page | these are lists, forms and sign-ins: what a platform's own controls do best, and what a person compares with the other apps on the device |
| the EDITORS stay web: everything under a project's address (`/@local/<workspace>/<project>`, the bench `@banc`, a surface opened alone `/@lan/…/@external/<surface>`) | the editor is one large drawn surface, shared by every platform; rewriting it per platform multiplies the work for no gain a person sees |
| Apple platforms only (iPhone, iPad, then the Mac); Android, Windows and Linux keep the web home | the mockups for them exist on the design canvas, but one platform at a time keeps the shared rules honest |
| native screens follow the design canvas "K7 DAW home, native UI test" (`Main` iOS 390×844, `IPad` iPadOS 1180×820, `Mac` macOS) for the look and the platform's idioms | the canvas is the living design system |
| the web home's behaviour and words are the reference for WHAT a page does (`apps/k7-daw-frontend/src/projects/`, `src/settings/`, `src/git/`, `apps/k7-daw-frontend/docs/projects.md`, `settings.md`, `git.md`, and the Interactions Sheet) | two homes that disagree are two products; a native page that needs a different behaviour changes the web's first |
| icons are SF Symbols, not lucide | `@studio/ui/icons` is the web's set; a native page draws the platform's own |

## Architecture

Every page talks to k7's Rust backend on the device over its local HTTP API, the routes the web home already calls.
Logic stays shared in the backend; only screens are rewritten.

| choice | why |
|---|---|
| one small Swift client, `K7Client` (`apps/k7-daw-ios/k7/K7Client.swift`), on `URLSession` | the routes are few and JSON; no dependency is worth the review |
| the client opens the door as the page does: `GET /auth/me` (which hands over the session cookie, `apps/k7-daw-backend/docs/sessions.md`), then the files hub's hello `GET /api/files-hub`, then the account's file service with the hello's ticket (`x-studio-user`, `x-studio-engine-token`, `x-studio-workspace`) | the same gate as the web, so the backend has no second way in to guard |
| a 401 from the file service asks the hub for a fresh ticket once | the ticket lives twelve hours; an app left open longer is the ordinary case |
| rules that decide what a list shows are pure Swift (`HomeRules`) mirroring the web's named functions, each saying which one | a rule found wrong is then fixed in both, and the tests read side by side |
| a view model per page (`HomeModel`), `@MainActor`, polling while the page is in front | the web home polls every 3 s for another device's change; the native page does the same and stops when hidden |
| the native views live in the iOS app's own folder for now; the Mac phase moves the shared ones to a Swift package the two targets compile | one target until there is a second one to share with |

### The handover between the native home and the web editor

The native home and k7's web view share one frame (`AppViews.swift`, `HomeHost.swift`). The web page stays loaded
under the native home.

| choice | why |
|---|---|
| the web view loads `/` at start, hidden under the native home | opening a project is then k7's in-place navigation, with no cold page load; the cost is the web home's own polling behind the native one |
| a native page opens an address in the web view by dispatching `k7-desktop-go` with the path | it is the desktop app's way to ask the page for an address (`apps/k7-daw-frontend/src/appNav.ts`): the page's own `go`, which asks about unsaved work; a page that has not heard it in 3 s is loaded at the address instead |
| the shell watches the web view's address (`WKWebView.url`, which a `pushState` changes too) and shows the native home when it is the bare home: path `/` with no query | leaving a project, by the page's Back, its home navigation, the edge swipe or the web tab bar's Projects, all end on `/`; nothing is injected into the page, so the web needs no change |
| `/` with a query stays web: `?sync=<ws>` (Sync settings), `?clone` (Clone), `?ws=<ws>` (a workspace's files), `?web` (the web home asked for by name) | those are web pages until their phase; `?web` is the web home's cards, which still do Save, Pull and the merge |
| tabs and menu entries for pages not native yet open their web route (`/surfaces`, `/tasks`, `/libraries`, `/settings`) | the web page's own tab bar brings the person back to `/`, so the native home |
| the app takes no deep link yet; when it does, a project address opens in the web view and a home address in the native home, by the same `HomeRules.isNativeHome` | one rule, the address's, decides who draws it |
| a surface opened on its own (`/@lan/<engine>/…/@external/<surface>`) is an editor address: web | it is a project's surface, not a home page |
| the language the native pages use is the web page's (its `PARAGLIDE_LOCALE`), read from the web view at each return home | k7's language is k7's own setting, not the system's; phase 2 moves it to the preferences so both read one place |
| the native home is dark, as the mockups | the web home's light theme comes with Settings, phase 2 |

## Localisation

| choice | why |
|---|---|
| the web's catalogues `apps/k7-daw-frontend/messages/en.json` and `fr.json` are bundled as they are and read at runtime (`L10n.swift`), not turned into a String Catalog | k7's language is k7's setting, which a String Catalog cannot follow (it follows the system's); a generated catalog is a generated file, and its `{name}` holes and `.one`/`.other` pairs would need converting anyway; read at runtime there is nothing to keep in step |
| the Xcode project names the folder from the frontend's sources | the catalogues stay one document in two languages, with the web's own test of their keys and holes |
| a word a native page needs and the web does not have is added to both catalogues | one place for every word of k7 |
| an XCTest reads the native sources for every key they name and checks both catalogues have it | a String Catalog would have checked at build; this is the same check |

## What stays web

The editors (every project address), the surfaces opened alone, and each home page until its phase lands. The web home
stays the home on Android, Windows and Linux, and in the browser.

## Testing

| choice | why |
|---|---|
| XCTest (`apps/k7-daw-ios/k7Tests`, hosted by the app) for the client against a stubbed `URLProtocol`, the rules, the view models and the catalogues | they decide everything a native page shows; a view is then a drawing of tested values |
| the app hosting the unit tests starts nothing: no backend, supervisor, engine or stdio relay (`AppDelegate.unitTestHost`) | the relay sends stdout to the system log, which XCTest reads back into stdout: a loop that never lets a test finish; the tests stub every route anyway |
| XCUITest for a native screen's flows comes with the Settings phase, where a screen's flow is more than a list | the Projects home has one gesture per element |
| the Playwright iOS target (`e2e/playwright.ios.config.ts`, `e2e/docs/e2e.md`) keeps driving the app's studio in the host's WebKit | it tests the routes and the editor, which stay shared; it never saw the app's own views |

## Phases

1. **The Projects home on iPhone and iPad.** Workspaces and their projects, a row opening the project in the web
   editor, New workspace and New project as native sheets, the header and tab bar of the mockups, the iPad's split view.
   The workspace's git line is read-only: Setup sync opens the web Sync settings, and Pull, Save and a waiting merge open
   the web home's cards (`/?web`). The native home is the app's start screen.
2. **Settings**, with its tabs, the Engine page and About; the language and theme move to the preferences.
3. **The git pages and the accounts**: Sync settings, Clone, Save, Pull and the merge screen, the Forgejo and GitHub
   sign-ins (`ASWebAuthenticationSession`, as `ForgeSignIn.swift` does already), "On your other devices".
4. **Tasks, Libraries with frozen chains, Surfaces**, and the first-project wizard.
5. **The Mac.** Needs the user's decision between:
   - **a native macOS SwiftUI target** that hosts what the Tauri app hosts: the Rust front in its process, the supervisor
     and the engine as programs, so VST3 and AU hosting and the helper programs (ffmpeg, the decoders) stay; the SwiftUI
     home shared with iOS through a package, the editor in a `WKWebView`;
   - **Mac Catalyst** of the iOS app: one target, but the iOS architecture comes with it, the engine a framework on a
     thread, no child programs, no plugin hosting, an iPad's controls on a Mac.

   **Recommended: the native macOS target.** A DAW on a Mac hosts plugins and runs helpers; Catalyst gives both up, and
   the Mac mockup is a Mac's sidebar and table, not an iPad's. It replaces the Tauri app on the Mac only; Windows and
   Linux keep Tauri and the web home.
6. **Removing the web home where native replaced it**, and only there: the web keeps every page a platform without a
   native one still shows, and the browser's.
