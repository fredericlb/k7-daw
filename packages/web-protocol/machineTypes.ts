// machineTypes.ts — what a machine's file service lists: workspaces, surfaces, served projects,
// stem jobs, module libraries. Re-exported by types.ts.

import type { ProjectApp } from "./projects.ts";
import type { FileNode } from "./workspaceTypes.ts";

/** A workspace under ~/.k7/root/<slug>/, as the picker lists them. */
export interface WorkspaceSummary {
  name: string; // the folder name (== the workspace name the user picks)
  projectCount: number; // project folders inside, at any depth (0 for empty)
  isSingleProject: boolean; // the folder itself is a project
}

/** Reply to GET /workspaces on the file service. */
export interface WorkspaceList {
  workspaces: WorkspaceSummary[];
}

/** One workspace on the machine with the projects it holds — the only side that can answer; see docs/machine.md. */
export interface AccountWorkspace {
  name: string;
  isSingleProject: boolean;
  projects: RepoProject[];
}

/** Reply to GET /workspaces/projects on the file service. */
export interface AccountWorkspaceList {
  workspaces: AccountWorkspace[];
}

/** One surface a project declares, named and typed — enough to build its address and to gate a device. */
export interface SurfaceRef {
  id: string;
  type: string;
}

/** One project declaring at least one surface, identity fields flattened onto it — see docs/machine.md. */
export interface SurfacedProject {
  workspace: string;
  path: string;
  app: ProjectApp;
  active: boolean;
  surfaces: SurfaceRef[];
}

/** Reply to GET /workspaces/surfaces; not @studio/song's `ProjectSurfaces` — see docs/machine.md. */
export interface SurfacedProjectList {
  projects: SurfacedProject[];
}

/** GET /engine/served — what the session actually LOADED, which is not `SurfacedProject.active`; see docs/machine.md. */
export interface ServedProject {
  workspace: string;
  /** Workspace-relative, `.` for a lone-project workspace — `SurfacedProject.path`'s spelling. */
  path: string;
}

/** Reply to GET /engine/served; `project: null` means the engine holds nothing for this person. */
export interface ServedProjectReply {
  project: ServedProject | null;
}

/** How a stem separation is doing, and the `files` it produced; see docs/machine.md. */
export interface SeparateJob {
  id: string;
  source: string; // the mix being split, absolute on the engine's machine
  done: boolean;
  failed: boolean;
  error: string | null;
  phase: string; // "queued" | "decoding" | "loading model" | "separating" | "writing" | "done" | "failed"
  percent: number | null; // null while doing something with no measurable end
  lines: string[]; // notes worth reading — which execution provider won, and why a run failed
  files: string[]; // the stems written, absolute. Empty until `done`.
}

/** What `POST /stems` answers: `ready` is the disk, `job` only when there was work — see docs/machine.md. */
export interface StemsReply {
  ready: boolean;
  job: SeparateJob | null;
}

/** One project inside a repository; the folder's NAME decides which app opens it — see ./projects.ts. */
export interface RepoProject {
  path: string;
  app: ProjectApp;
  /** `project.tags` of an k7 project's project.k7x; absent when it has none. */
  tags?: string[];
  /** The TYPES of the surfaces it declares (`timeline`, `turntables`…), which is what its archetype was; absent from an older service. */
  surfaces?: string[];
  /** Whether it is its workspace's active project, the one the engine plays — docs/machine.md. */
  active?: boolean;
  /** When a file in it last changed on this replica, epoch ms, from a walk two levels deep. */
  editedAt?: number;
  /** A bundled demo, overwritten on the service's next start — the home page says so; absent otherwise. */
  demo?: boolean;
}

/** One module LIBRARY a project can reach, nearest first; `exists: false` is a state, not an absence — docs/machine.md. */
export interface ModuleLibrary {
  /** The `@`-prefixed level a chain writes: `workspace`, `k7`, `engine`. */
  ns: string;
  /** Workspace-relative, so every other route takes it as-is; EMPTY for `@engine`, which has no folder. */
  path: string;
  exists: boolean;
  /** Nothing here can be created, renamed or written — true for `@engine` alone; see docs/machine.md. */
  readOnly?: boolean;
  nodes: FileNode[];
  /** A library's shared SOUNDS, beside its modules — what a tap and a cabinet pick, and where a drop lands (@studio/song's libraryFiles.md). */
  samples?: FileNode[];
  irs?: FileNode[];
  /** …its WAVETABLES, what a Faust instrument's table slot picks (@studio/song's faustFiles.md). */
  wavetables?: FileNode[];
  /** …and its shared amp MODELS, what an amp's `model` may pick besides its own assets. */
  models?: FileNode[];
  /** For one of the origin's libraries: the version this workspace plays, and the newest this k7 could move to. */
  version?: { pinned: number | "draft"; requires?: number; upgrade?: number };
}
