// appMain.tsx — k7 itself: the gates, the router and the editor; main.tsx imports it unless the address is the
// documentation host, so the docs never load the editor. The router is mounted by the address's SCOPE
// (address.ts), so going into a project or out of one swaps the tree without a page load (appNav.ts).

import { wasmReady } from "@studio/k7-wasm/boot";
import {
  setFileApiRevision,
  setFileApiTally,
} from "@studio/project-shell/fileApi";
import { GateWait, releaseSplash } from "@studio/project-shell/progress";
import { clearDevPerfEntries } from "@studio/ui/dev-perf";
import { DialogCloseLabel } from "@studio/ui/dialog";
import { registerWorker } from "@studio/vite-config/register";
import { projectLabel } from "@studio/web-protocol/projects";
import { QueryClientProvider } from "@tanstack/react-query";
import {
  lazy,
  Profiler,
  type ReactNode,
  Suspense,
  useEffect,
  useLayoutEffect,
  useState,
  useSyncExternalStore,
} from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { type Scope, scopeOf } from "./address";
import { api, engines } from "./api";
import { currentScopeKey, subscribeAddress, useScopeWatch } from "./appNav";
import { AuthGate } from "./components/AuthGate";
import "./disabledReasonToast";

/** The four screens that are NOT the home page, split out of the entry — see docs/shell.md.
 *  Only one address is ever taken, so only one of these is ever fetched. */
const loadApp = () => import("./App");
const loadEngineGate = () => import("./components/EngineGate");
const EngineGate = lazy(() =>
  loadEngineGate().then((m) => ({ default: m.EngineGate })),
);
const ExternalIndex = lazy(() =>
  import("./components/ExternalIndex").then((m) => ({
    default: m.ExternalIndex,
  })),
);
/** The home page and its sibling pages: a project page never loads them. */
const loadHomeRoutes = () => import("./projects/HomeRoutes");
const HomeRoutes = lazy(() =>
  loadHomeRoutes().then((m) => ({ default: m.HomeRoutes })),
);

import { PROFILING } from "@studio/project-shell/profiling";
import { shell } from "./appShell";
import { installFreezeWatchdog, reportCommit, tally } from "./freezeWatchdog";
import { currentRevision } from "./hooks/useRevision";
import { LocaleProvider, useLocale } from "./i18n";
import { installLongPress } from "./longPress";
import { PaneBoundary } from "./PaneBoundary";
import { m } from "./paraglide/messages.js";
import { queryClient } from "./queries/client";
import { forgetLeftScope } from "./scopeLeave";
import { followSystemTextSize } from "./textScale";
import { spawnBrowserWorker } from "./verbs/browserWorker";
import { installPageVerbs } from "./verbs/pageHost";

import "./index.css";
/** The waveform's own wrapper and marker rules — a plain stylesheet, so the `@source` scan cannot produce it and it has to be imported. */
import "@studio/ui-waveform/styles.css";

// Report main-thread stalls to the console, so a "random freeze" leaves evidence. Cheap, and console-only.
installFreezeWatchdog({ commits: PROFILING });
// …and let the file transport report into the same tally: parsing a response is the other thing
// that runs on this thread while a project opens, and the engine link was the only one instrumented.
setFileApiTally(tally);
// The compiled views follow the change token alone, so a pane mounting a moment after another is answered from its read.
setFileApiRevision(currentRevision, (path) =>
  /^\/(graph|project-doc|project-plugins)(\?|$)/.test(path),
);
// The command bar's verbs run in a Worker the first call starts (docs/verbs.md).
installPageVerbs(spawnBrowserWorker);
// A held finger or pen is a right-click on every surface, an iPad included (docs/contextMenu.md).
installLongPress();

/** The Profiler is here for the freeze watchdog, and OFF unless someone is looking: it bills every commit in a tree that commits per meter frame. */
const profiling = PROFILING;

const BASE = import.meta.env.BASE_URL;

// What the page wears before any scope dresses it, put back when a scope does not.
const PAGE_TITLE = document.title;

/** Dress the page for its scope: its title off the ADDRESS, right before anything loads; `App` refines it to the song's own name. */
function dressPage(scope: Scope): void {
  document.title =
    scope.kind === "project"
      ? scope.at.project === "."
        ? scope.at.repo
        : projectLabel(scope.at.project.split("/").pop() ?? "")
      : PAGE_TITLE;
}
dressPage(scopeOf(window.location.pathname, BASE));

/** …and the first screen's files start arriving now, while AuthGate asks who you are, not once it has its answer (docs/shell.md). */
const first = scopeOf(window.location.pathname, BASE).kind;
// A failure here is the lazy screen's to report when it asks again, not an unhandled rejection.
if (first === "home") loadHomeRoutes().catch(() => {});
else if (first === "project") {
  loadEngineGate().catch(() => {});
  // The editor is the longest download of any start: begun here, not once ProjectGate mounts after the sign-in check.
  loadApp().catch(() => {});
}
// The file hub's hello needs the cookie, which /auth/me hands over: it asks once that answered (`@studio/engine-browser/access`).
if (first === "home" || first === "project")
  void api.getMe().then(
    () => engines.hostedFiles(),
    () => {},
  );

/** …and the worker that makes either an installable app — a SECOND opening, not offline; its scope is `/`, and each deploy replaces it (@studio/vite-config/register). */
// Not in an app's view: its files are already on the machine, and the precache would serve the previous release's page after an update (docs/localApp.md).
if ("serviceWorker" in navigator) {
  if (!shell.inApp) registerWorker();
  else void dropWorkers();
}

/** An app's view drops any registration it still holds, and the precache goes with it. */
async function dropWorkers(): Promise<void> {
  const all = await navigator.serviceWorker.getRegistrations();
  await Promise.all(all.map((r) => r.unregister()));
}

/** What is on screen while a split-out screen arrives: at startup, under the splash; later, the bar alone (apps/k7-daw-frontend/docs/shell.md). */
function Booting() {
  return (
    <div className="min-h-screen bg-background">
      <GateWait label={m["startup.loading"]()} />
    </div>
  );
}

// React's DEV build leaves a `performance.measure` per render that the browser keeps forever — cleared (docs/shell.md).
clearDevPerfEntries(import.meta.env.DEV);

/** The router of ONE scope, keyed by it: the project is its basename, so React Router only writes below it and cannot rewrite the project out. */
function Scoped({
  scope,
  ready,
  children,
}: {
  scope: Scope;
  /** False for the one render between two scopes: the last one is gone, the next one not yet drawn. */
  ready: boolean;
  children: ReactNode;
}) {
  return (
    <BrowserRouter key={scope.key} basename={scope.basename}>
      <ScopeWatch />
      {ready ? children : null}
    </BrowserRouter>
  );
}

function ScopeWatch() {
  useScopeWatch();
  return null;
}

/** What the address is, drawn — remounted between scopes, with what the last one kept forgotten in between (appNav.ts). */
function Root() {
  // After the first commit, the children's waits have held the splash already: none, and it goes (docs/icons.md).
  useEffect(releaseSplash, []);
  const key = useSyncExternalStore(subscribeAddress, currentScopeKey);
  const [shown, setShown] = useState(key);
  // Two steps: the old scope unmounts on this render, the page forgets it, and only then does the new one mount.
  const ready = shown === key;
  useLayoutEffect(() => {
    if (ready) return;
    forgetLeftScope();
    dressPage(scopeOf(window.location.pathname, BASE));
    setShown(key);
  }, [key, ready]);
  const scope = scopeOf(window.location.pathname, BASE);
  return (
    // The last line: a fault that escaped every pane's own boundary draws a card here, not a white page.
    <PaneBoundary name="k7" resetKey={scope.key}>
      {/* Above the router, so moving between scopes does not ask who you are again. */}
      <AuthGate>
        <Scoped scope={scope} ready={ready}>
          <Suspense fallback={<Booting />}>
            {scope.kind === "external-index" ? (
              // The second device's front door: every surface this account can drive, holding no engine — a lease would evict the person playing.
              <ExternalIndex />
            ) : scope.kind === "home" ? (
              // The pages with no project are ROUTES of one scope, so going between them remounts nothing; a take outlives them all.
              <HomeRoutes fallback={<Booting />} />
            ) : profiling ? (
              <Profiler id="app" onRender={reportCommit}>
                <EngineGate external={scope.external} />
              </Profiler>
            ) : (
              <EngineGate external={scope.external} />
            )}
          </Suspense>
        </Scoped>
      </AuthGate>
    </PaneBoundary>
  );
}

/** Gives every kit dialog its close button's name in the language on show. */
function DialogLabels({ children }: { children: ReactNode }) {
  useLocale();
  return (
    <DialogCloseLabel.Provider value={m["common.close"]()}>
      {children}
    </DialogCloseLabel.Provider>
  );
}

followSystemTextSize();

// The gates before the editor, and NO React.StrictMode — its double-mount freezes Firefox's WebGL context (docs/shell.md).
wasmReady().finally(() => {
  // Mounted once the module's compile settles, whatever its outcome: a curve drawn before it throws `unavailable`.
  ReactDOM.createRoot(document.getElementById("root")!, {
    // What the boundaries catch, and what nothing did, both reach the console with their component stack.
    onCaughtError: (error, info) =>
      console.error("[caught]", error, info.componentStack),
    onUncaughtError: (error, info) =>
      console.error("[uncaught]", error, info.componentStack),
  }).render(
    // One cache for the page: the panes' shared reads, keyed by what they follow (queries/client.ts) — and cleared between projects.
    <QueryClientProvider client={queryClient}>
      <LocaleProvider>
        <DialogLabels>
          <Root />
        </DialogLabels>
      </LocaleProvider>
    </QueryClientProvider>,
  );
});
