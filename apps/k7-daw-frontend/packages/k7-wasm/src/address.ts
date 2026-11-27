// address.ts — a project's address read and written by the Rust crate (song-schema's address.rs); the names it reserves are generated constants.
// A reader would get wrong that no origin is here: a path comes back, and the page joins its own origin (web-protocol's projects.ts).

import { call } from "./abi.ts";
import type { BenchRef, LanSurface, Launch, LocalTarget } from "./types/index.ts";

export {
  ABOUT_PAGE,
  BENCH_PROJECT,
  BENCH_REPO,
  BENCH_WORKSPACE,
  DROPPABLE_PROJECTS,
  EXAMPLES_WORKSPACE,
  EXTERNAL_SEGMENT,
  LAN_REPO,
  LIBRARIES_PAGE,
  LOCAL_REPO,
  MEDIA_PAGE,
  PROJECT_SUFFIX,
  SETTINGS_PAGE,
  SURFACES_PAGE,
  TASKS_PAGE,
} from "./consts.generated.ts";
export type { LanSurface, Launch, LocalTarget };

const name = <T>(op: string, value: string): T =>
  call<T>({ op: `address.${op}`, args: { name: value } });
const at = <T>(op: string, pathname: string, base: string): T =>
  call<T>({ op: `address.${op}`, args: { pathname, base } });

/** Read `/<repo>/<project…>[/<view…>]` under `base`, or null — docs/addresses.md. */
export const parseLaunch = (pathname: string, base: string): Launch | null =>
  at("parse_launch", pathname, base);

/** The router basename: a slice of `pathname` up to the project's end, `base` when it names none. */
export const projectBasename = (pathname: string, base: string): string =>
  at("basename", pathname, base);

/** The surface an address opens on its own, or null. */
export const parseExternal = (pathname: string, base: string): string | null =>
  at("parse_external", pathname, base);

/** A `@lan` surface address read back, or null for anything else. */
export const parseLanSurface = (
  pathname: string,
  base: string,
): LanSurface | null => at("parse_lan_surface", pathname, base);

/** Whether an address is a page that hangs off the home, or below one. */
export const isHomePage = (pathname: string, base: string): boolean =>
  at("is_home_page", pathname, base);

/** `/<repo>/<project>`, each segment encoded; the project `.` adds none. */
export const launchPath = (repo: string, project: string): string =>
  call<string>({ op: "address.launch_path", args: { repo, project } });

/** A surface hung off a project URL somebody else built. */
export const externalOf = (projectUrl: string, surface: string): string =>
  call<string>({ op: "address.external_of", args: { projectUrl, surface } });

/** The path of a surface of another device. */
export const lanSurfacePath = (
  engineId: string,
  workspace: string,
  project: string,
  surface: string,
): string =>
  call<string>({
    op: "address.lan_surface_path",
    args: { engineId, workspace, project, surface },
  });

/** The path of a project in a workspace with no repository. */
export const localProjectPath = (workspace: string, project: string): string =>
  call<string>({ op: "address.local_project_path", args: { workspace, project } });

/** Split the project half of a `@local` address, checked not trusted. */
export const localTarget = (project: string): LocalTarget | null =>
  name("local_target", project);

/** The library chain a bench project names: null for the bare rig, undefined for no bench. */
export function benchChainOf(project: string): string | null | undefined {
  const bench = name<BenchRef | null>("bench_chain", project);
  return bench ? bench.chain : undefined;
}

/** The project half of a bench address, seeded with `chain` when one is given. */
export const benchProject = (chain?: string | null): string =>
  call<string>({ op: "address.bench_project", args: { chain: chain ?? null } });

/** How a project is named in an engine claim. */
export const projectClaimKey = (l: Launch): string =>
  call<string>({ op: "address.claim_key", args: l });

/** Whether a claimed address plays in the bench's engine session. */
export const claimIsBench = (claim: string): boolean =>
  name("claim_is_bench", claim);

/** Whether a folder NAME is a project's. */
export const isProjectName = (folder: string): boolean =>
  name("is_project", folder);

/** The folder name with its suffix taken off. */
export const projectLabel = (folder: string): string =>
  name("project_label", folder);

/** Whether a file NAME is a project a drop may carry. */
export const isDroppableProject = (file: string): boolean =>
  name("is_droppable", file);
