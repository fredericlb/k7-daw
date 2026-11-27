// projects.ts — the project address as the page uses it: the grammar is Rust's (`@studio/k7-wasm/address`), this file joins the origin to it.
// A reader would get wrong that `app` stays a parameter only for the callers' sake: k7 owns the one suffix, and Rust does not ask.
// See apps/k7-daw-frontend/docs/projects.md and apps/k7-daw-frontend/packages/project-shell/docs/launch.md.

import {
  BENCH_REPO,
  benchChainOf,
  claimIsBench,
  externalOf,
  isProjectName,
  type LanSurface,
  LOCAL_REPO,
  lanSurfacePath,
  launchPath,
  localProjectPath,
  PROJECT_SUFFIX,
  parseExternal as parseExternalIn,
  parseLanSurface as parseLanSurfaceIn,
  parseLaunch as parseLaunchIn,
  projectBasename as projectBasenameIn,
  SURFACES_PAGE,
} from "@studio/k7-wasm/address";
import { type StudioService, serviceUrl } from "./services.ts";

export {
  ABOUT_PAGE,
  BENCH_PROJECT,
  BENCH_REPO,
  BENCH_WORKSPACE,
  benchChainOf,
  benchProject,
  DROPPABLE_PROJECTS,
  EXAMPLES_WORKSPACE,
  EXTERNAL_SEGMENT,
  isDroppableProject,
  isHomePage,
  LAN_REPO,
  type LanSurface,
  type Launch,
  LIBRARIES_PAGE,
  LOCAL_REPO,
  localTarget,
  MEDIA_PAGE,
  projectClaimKey,
  projectLabel,
  SETTINGS_PAGE,
  SURFACES_PAGE,
  TASKS_PAGE,
} from "@studio/k7-wasm/address";

/** The apps a project folder can belong to. A subset of the services: k7 owns no projects. */
export type ProjectApp = Extract<StudioService, "k7">;

/** The suffix each app claims — one, because everything an k7 project can BE is a shape INSIDE `.k7`. */
export const PROJECT_SUFFIXES: Record<ProjectApp, string> = {
  k7: PROJECT_SUFFIX,
};

/** The app that opens a folder with this NAME, which is not the question of whether it compiles — docs/addresses.md. */
export const projectApp = (name: string): ProjectApp | null =>
  isProjectName(name) ? "k7" : null;

/** Read `/<repo>/<project…>[/<view…>]` into a target, or null — the rules are in docs/addresses.md. */
export const parseLaunch = (pathname: string, _app: ProjectApp, base: string) =>
  parseLaunchIn(pathname, base);

/** The router `basename`: a SLICE of the given path, never a rebuild of it — see docs/addresses.md. */
export const projectBasename = (
  pathname: string,
  _app: ProjectApp,
  base: string,
): string => projectBasenameIn(pathname, base);

/** Which surface this address opens on its own, off the address alone; null for a bare `@external`. */
export const parseExternal = (
  pathname: string,
  _app: ProjectApp,
  base: string,
): string | null => parseExternalIn(pathname, base);

/** Read a `@lan` surface address, or null for anything else — an `@lan` with no surface after it is not one. */
export const parseLanSurface = (
  pathname: string,
  _app: ProjectApp,
  base: string,
): LanSurface | null => parseLanSurfaceIn(pathname, base);

/** Whether an address names the bench rather than a repository. */
export const isBench = (repo: string): boolean => repo === BENCH_REPO;

/** Whether an address names a workspace on the machine rather than a repository. */
export const isLocal = (repo: string): boolean => repo === LOCAL_REPO;

/** Whether the project half of a `@banc` address names that rig, bare or seeded with a chain. */
export const isBenchProject = (project: string): boolean =>
  benchChainOf(project) !== undefined;

/** Which of an account's engine sessions a claimed address plays in: the bench has its own — docs/addresses.md. */
export const claimSlot = (claim: string): "bench" | "project" =>
  claimIsBench(claim) ? "bench" : "project";

/** Where a project opens — the link pl8 writes; only the ORIGIN of `origin` is kept, so pass anything. */
export function launchUrl(
  origin: string,
  repo: string,
  project: string,
  app: ProjectApp,
): string {
  return serviceUrl(origin, app) + launchPath(repo, project);
}

/** …and a `@local` project's address, built like every other. */
export const localProjectUrl = (
  origin: string,
  workspace: string,
  project: string,
  app: ProjectApp,
): string => serviceUrl(origin, app) + localProjectPath(workspace, project);

/** The link to hand another device, from the origin the TAB is on — a tablet by the decks is on the LAN one. */
export const externalUrl = (
  origin: string,
  l: { repo: string; project: string },
  surface: string,
  app: ProjectApp,
): string => externalOf(launchUrl(origin, l.repo, l.project, app), surface);

/** Where a surface of another device opens: below the project like `externalUrl`, under that device's id. */
export const lanSurfaceUrl = (
  origin: string,
  l: Omit<LanSurface, "surface">,
  surface: string,
  app: ProjectApp,
): string =>
  serviceUrl(origin, app) +
  lanSurfacePath(l.engineId, l.workspace, l.project, surface);

/** The whole URL of the SURFACES page: a surface cannot use the router, its basename ending at the project. */
export const surfacesUrl = (origin: string, app: ProjectApp): string =>
  `${serviceUrl(origin, app)}/${SURFACES_PAGE}`;

export { externalOf };
