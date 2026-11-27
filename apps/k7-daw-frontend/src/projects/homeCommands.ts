// homeCommands.ts — the HOME page's own command vocabulary: what the bar under the projects
// list offers, and what a line does. None of them the editor's — this screen has no
// project, no engine session and no file open, so `LOCAL_COMMANDS` would be a catalogue of
// things that cannot be done here. Pure but for the acts it is handed. See ../../docs/projects.md.

import { PROJECT_SHAPES } from "@studio/k7-wasm/consts";
import {
  homeCatalogue,
  homeFindProject,
  homeGripe,
  homeParse,
  homeResolve,
  homeArgValues as valuesOf,
} from "@studio/k7-wasm/menu";
import type { ProjectShape } from "@studio/k7-wasm/types";
import type { AccountWorkspace, RepoProject } from "@studio/web-protocol";
import type { PaletteCommand } from "../components/CommandBar";
import { words } from "../menuWords";
// `m` is the file service in these branches, so the catalogue comes in under its own name.
import { m as msg } from "../paraglide/messages.js";
import type { AccountFiles } from "./api";
import { projectFolderName, workspaceFolderName } from "./projectName";
import {
  copyProject,
  createProject,
  createWorkspace,
  deleteProject,
  renameProject,
} from "./workspaceOps";

/** The open workspace travels in the QUERY, not in the path: the path belongs to launching, and a query bookmarks and answers Back. */
export const WS_PARAM = "ws";

/** The catalogue as the bar takes it: Rust's rows, each hint resolved in the app's language. */
export function buildHomePalette(locale: string): PaletteCommand[] {
  return homeCatalogue(locale).map((r) => ({
    canonical: r.canonical,
    label: r.label,
    names: r.names,
    readable: r.readable,
    readableBy: { ...r.readableBy },
    args: r.args.map((a) => ({
      name: a.name,
      kind: a.kind as PaletteCommand["args"][number]["kind"],
      hint: words(a.hint),
      ...(a.optional ? { optional: true } : {}),
    })),
  }));
}

/** What this screen knows when a line is run: the file service, what it holds, and what pl8 registers. */
export interface HomeContext {
  fileService: AccountFiles | null;
  workspaces: AccountWorkspace[];
  /** The workspace the address has unfolded (`?ws=`), or null. */
  openWs: string | null;
}

/** Which workspace a `project …` line acts in: the address first, then an account holding exactly one; anything else refused by name. */
export function resolveWorkspace(
  ctx: HomeContext,
): { ws: AccountWorkspace } | { error: string } {
  const r = homeResolve(ctx);
  return r.is === "workspace"
    ? { ws: ctx.workspaces[r.index] as AccountWorkspace }
    : { error: words(r.text) };
}

/** A project of that workspace, named by its path or by the folder's stem (`hiver`). */
export function findProject(
  ws: AccountWorkspace,
  name: string,
): RepoProject | null {
  const at = homeFindProject(ws, name);
  return at === null ? null : (ws.projects[at] ?? null);
}

/** One param's whole vocabulary, for the field that is open: answered synchronously, this page already polling both services. */
export const homeArgValues = (
  canonical: string,
  argIndex: number,
  ctx: HomeContext,
): string[] => valuesOf(canonical, argIndex, ctx);

/** The project a `project delete init` has named, waiting for its confirm. */
export interface ArmedDelete {
  workspace: string;
  path: string;
}

/** What a line DOES, injected — so the dispatcher below is testable without a file service, and the bar and the buttons cannot diverge. */
export interface HomeActs {
  create: typeof createProject;
  copy: typeof copyProject;
  rename: typeof renameProject;
  remove: typeof deleteProject;
  newWorkspace: typeof createWorkspace;
}

export const HOME_ACTS: HomeActs = {
  create: createProject,
  copy: copyProject,
  rename: renameProject,
  remove: deleteProject,
  newWorkspace: createWorkspace,
};

export interface HomeDeps extends HomeContext {
  acts: HomeActs;
  /** What happened, or where a long act has got to — the line under the bar. */
  say: (text: string) => void;
  /** …and the same channel for a NO — two of them, because a screen cannot otherwise tell them apart. */
  refuse: (text: string) => void;
  /** Re-read both services without waiting for the poll: after a write we already know. */
  changed: () => void;
  /** The project `project delete init` named, held between the two halves — a ref, CONSUMED by every line, the confirm or not. */
  armed: { current: ArmedDelete | null };
  /** Turn this device into a camera under that name, null for its own — or the sentence saying why it cannot. */
  cameraMode?: (name: string | null) => string | null;
  /** Open the documentation over the page. */
  help?: () => void;
  /** Go to a workspace's files, or to one of its projects. */
  open?: (workspace: string, project?: RepoProject) => void;
}

/** The file service, or the sentence saying why there is nothing to act on. */
const machineOf = (d: HomeDeps): AccountFiles | null => {
  if (d.fileService) return d.fileService;
  d.refuse(msg["hc.noFiles"]());
  return null;
};

/** Run one line of this screen's vocabulary; `false` for a line no verb here matches, so the caller says 'unknown command' once. */
export async function runHomeCommand(
  raw: string,
  d: HomeDeps,
): Promise<boolean> {
  const parsed = homeParse(raw);
  if (parsed.is === "empty") return true;
  if (parsed.is === "unknown") return false;
  const rest = parsed.rest;
  // Consumed HERE, before any branch: whatever this line is, the arming belonged to the one before it.
  const armed = d.armed.current;
  d.armed.current = null;

  switch (parsed.canonical) {
    case "workspace open": {
      const ws = d.workspaces.find((w) => w.name === rest);
      if (!ws) return use(d.refuse, msg["hc.noWorkspaceNamed"]({ name: rest }));
      d.open?.(ws.name);
      return true;
    }

    case "project open": {
      const cut = rest.indexOf("/");
      const ws = d.workspaces.find((w) => w.name === rest.slice(0, cut));
      const p = ws && cut > 0 ? findProject(ws, rest.slice(cut + 1)) : null;
      if (!ws)
        return use(
          d.refuse,
          msg["hc.noWorkspaceNamed"]({ name: rest.slice(0, cut) || rest }),
        );
      if (!p) return use(d.refuse, projectGripe(ws, rest.slice(cut + 1)));
      d.open?.(ws.name, p);
      return true;
    }

    case "project new": {
      const [type, ...words] = rest.split(/\s+/).filter(Boolean);
      const name = words.join(" ");
      if (!type || !name) return use(d.refuse, msg["hc.usage.projectNew"]());
      if (!(PROJECT_SHAPES as readonly string[]).includes(type))
        return use(
          d.refuse,
          msg["hc.notAShape"]({ type, shapes: PROJECT_SHAPES.join(", ") }),
        );
      const { folder, error } = projectFolderName(name, "k7");
      if (error) return use(d.refuse, error);
      const m = machineOf(d);
      if (!m) return true;
      const found = resolveWorkspace(d);
      if ("error" in found) return use(d.refuse, found.error);
      try {
        const { project, missing } = await d.acts.create(
          m,
          found.ws.name,
          folder,
          d.say,
          type as ProjectShape,
        );
        d.changed();
        d.say(
          missing.length
            ? msg["project.missingModules"]({
                project,
                modules: missing.join(", "),
              })
            : msg["hc.projectMade"]({ path: `${found.ws.name}/${project}` }),
        );
      } catch (e) {
        d.refuse(
          msg["hc.failed"]({
            command: "project new",
            reason: (e as Error).message,
          }),
        );
      }
      return true;
    }

    case "project copy": {
      const [name, to, ...words] = rest.split(/\s+/).filter(Boolean);
      if (!name || !to) return use(d.refuse, msg["hc.usage.projectCopy"]());
      const found = resolveWorkspace(d);
      if ("error" in found) return use(d.refuse, found.error);
      const p = findProject(found.ws, name);
      if (!p) return use(d.refuse, projectGripe(found.ws, name));
      const dest = d.workspaces.find((w) => w.name === to);
      if (!dest) return use(d.refuse, msg["hc.noWorkspaceNamed"]({ name: to }));
      // Said rather than attempted: the file service refuses both too, and later.
      if (dest.name === found.ws.name)
        return use(d.refuse, msg["hc.copySameWs"]({ to }));
      if (dest.isSingleProject)
        return use(d.refuse, msg["hc.copyIntoProject"]({ to }));
      // No name given is the source's own, suffix included, which `projectFolderName` puts back.
      const label = words.join(" ") || (p.path.split("/").pop() ?? p.path);
      const { folder, error } = projectFolderName(label, p.app);
      if (error) return use(d.refuse, error);
      const m = machineOf(d);
      if (!m) return true;
      try {
        await d.acts.copy(m, found.ws.name, p.path, dest.name, folder, d.say);
        d.changed();
        d.say(msg["hc.copied"]({ path: `${to}/${folder}` }));
      } catch (e) {
        d.refuse(
          msg["hc.failed"]({
            command: "project copy",
            reason: (e as Error).message,
          }),
        );
      }
      return true;
    }

    case "project rename": {
      const [name, ...words] = rest.split(/\s+/).filter(Boolean);
      const label = words.join(" ");
      if (!name || !label)
        return use(d.refuse, msg["hc.usage.projectRename"]());
      const found = resolveWorkspace(d);
      if ("error" in found) return use(d.refuse, found.error);
      const p = findProject(found.ws, name);
      if (!p) return use(d.refuse, projectGripe(found.ws, name));
      const { folder, error } = projectFolderName(label, p.app);
      if (error) return use(d.refuse, error);
      // Said rather than attempted: the file service would answer the same thing, one round trip later.
      if (folder === p.path.split("/").pop())
        return use(d.refuse, msg["hc.renameSame"]({ name: folder }));
      const m = machineOf(d);
      if (!m) return true;
      try {
        await d.acts.rename(m, found.ws.name, p.path, folder, d.say);
        d.changed();
        d.say(msg["hc.renamed"]({ from: p.path, to: folder }));
      } catch (e) {
        d.refuse(
          msg["hc.failed"]({
            command: "project rename",
            reason: (e as Error).message,
          }),
        );
      }
      return true;
    }

    case "project delete init": {
      if (!rest) return use(d.refuse, msg["hc.usage.deleteInit"]());
      const found = resolveWorkspace(d);
      if ("error" in found) return use(d.refuse, found.error);
      const p = findProject(found.ws, rest);
      if (!p) return use(d.refuse, projectGripe(found.ws, rest));
      d.armed.current = { workspace: found.ws.name, path: p.path };
      // What the dialog's paragraph says, said here instead: 'are you sure?' on its own names nothing.
      d.say(msg["hc.deleteArmed"]({ path: p.path }));
      return true;
    }

    case "project delete confirm": {
      if (!rest) return use(d.refuse, msg["hc.usage.deleteConfirm"]());
      if (!armed) return use(d.refuse, msg["hc.nothingArmed"]({ name: rest }));
      const found = resolveWorkspace(d);
      if ("error" in found) return use(d.refuse, found.error);
      const p = findProject(found.ws, rest);
      // NAMED AGAIN, and it has to be the same one, or the second line would be the formality it must not be.
      if (!p || p.path !== armed.path || found.ws.name !== armed.workspace)
        return use(
          d.refuse,
          msg["hc.notWhatWasArmed"]({
            what: `${armed.workspace}/${armed.path}`,
          }),
        );
      const m = machineOf(d);
      if (!m) return true;
      try {
        await d.acts.remove(m, armed.workspace, p.path, d.say);
        d.changed();
        d.say(msg["hc.deleted"]({ path: `${armed.workspace}/${p.path}` }));
      } catch (e) {
        d.refuse(
          msg["hc.failed"]({
            command: "project delete",
            reason: (e as Error).message,
          }),
        );
      }
      return true;
    }

    case "help":
      d.help?.();
      return true;

    case "system workspace new": {
      if (!rest) return use(d.refuse, msg["hc.usage.workspaceNew"]());
      const { folder, error } = workspaceFolderName(rest);
      if (error) return use(d.refuse, error);
      const m = machineOf(d);
      if (!m) return true;
      try {
        const { workspace } = await d.acts.newWorkspace(m, folder, d.say);
        d.changed();
        // Said out loud: it holds nothing, and the editor is where the first project is chosen.
        d.say(msg["hc.workspaceMade"]({ workspace }));
      } catch (e) {
        d.refuse(
          msg["hc.failed"]({
            command: "system workspace new",
            reason: (e as Error).message,
          }),
        );
      }
      return true;
    }
  }
  return false;
}

/** No such project, with the ones there are: a refusal that names nothing is a refusal you retype. */
const projectGripe = (ws: AccountWorkspace, name: string): string =>
  words(homeGripe(ws, name));

/** Say it and answer `true` — the branches above are one statement each because of it. */
function use(to: (s: string) => void, text: string): boolean {
  to(text);
  return true;
}
