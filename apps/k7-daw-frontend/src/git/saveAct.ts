// saveAct.ts — Save, one press: everything in the workspace is committed under a dated message, then pushed; a toast says each step.
// What a reader gets wrong: a push the remote refuses as behind leaves the save on this device, and the toast offers the Pull.

import { saveWhen } from "@studio/k7-wasm/git";
import {
  dismissToast,
  showToast,
  updateToast,
} from "@studio/project-shell/toast";
import { useSyncExternalStore } from "react";
import { m } from "../paraglide/messages.js";
import { commitView, formatSize, identityReady } from "./commitState";
import { explain, GitError, type GitTransport, git } from "./gitApi";
import { announceGitMoved } from "./gitBus";
import { remoteName } from "./gitState";

/** The message a save is recorded under: the moment, as the canvas writes it (23/11/2026 00:14). */
export function saveMessage(at: Date): string {
  return m["git.save.message"]({ when: saveWhen(at) });
}

const saving = new Set<string>();
const listeners = new Set<() => void>();
const changed = () => {
  for (const l of listeners) l();
};

/** Whether a save of `workspace` is running, so every Save button of it dims. */
export function useSaving(workspace: string): boolean {
  return useSyncExternalStore(
    (fn) => {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    () => saving.has(workspace),
  );
}

export interface SaveHooks {
  /** The toast's Pull, after a push refused as behind. */
  onPull: () => void;
}

const count = (
  one: (a: { n: number }) => string,
  other: (a: { n: number }) => string,
  n: number,
) => (n === 1 ? one : other)({ n });
const filesText = (n: number) =>
  count(m["git.files.one"], m["git.files.other"], n);

// Loaded on use: the address helpers bring the editor's buffers along.
const openSettings = () =>
  void import("../appNav").then((nav) =>
    nav.go(`${import.meta.env.BASE_URL.replace(/\/?$/, "/")}settings`),
  );

/** Commit everything (but what no host takes) and push, saying each step in one toast; a second press while it runs is ignored. */
export async function save(
  t: GitTransport,
  workspace: string,
  hooks: SaveHooks,
): Promise<void> {
  if (saving.has(workspace)) return;
  saving.add(workspace);
  changed();
  try {
    await run(t, workspace, hooks);
  } finally {
    saving.delete(workspace);
    changed();
  }
}

async function run(t: GitTransport, workspace: string, hooks: SaveHooks) {
  const again = () => void save(t, workspace, hooks);
  let entries: Awaited<ReturnType<typeof git.status>>["entries"];
  let state: Awaited<ReturnType<typeof git.state>>;
  let who: Awaited<ReturnType<typeof git.identity>> | null;
  try {
    [{ entries }, state, who] = await Promise.all([
      git.status(t, workspace),
      git.state(t, workspace),
      git.identity(t).catch(() => null),
    ]);
  } catch (e) {
    showToast({
      title: m["git.save.failed"](),
      message: explain(e),
      variant: "error",
      action: { label: m["git.save.retry"](), onClick: again },
    });
    return;
  }
  const view = commitView(entries);
  const sent = view.sent.length;
  const connected = state.mode === "connected";
  const pushOnly = sent === 0 && connected && (state.ahead ?? 0) > 0;
  if (sent === 0 && !pushOnly) {
    showToast({ message: m["git.noChanges"](), variant: "neutral" });
    return;
  }
  const target = state.remote
    ? `${remoteName(state.remote)} · ${state.branch ?? ""}`
    : "";
  const message = saveMessage(new Date());
  const files = filesText(sent);
  const sending =
    view.lfs > 0 ? m["git.save.filesLarge"]({ files, large: view.lfs }) : files;

  if (!pushOnly && !identityReady(who)) {
    showToast({
      title: m["git.save.noAuthor"](),
      message: m["git.save.noAuthorBody"](),
      variant: "warning",
      action: { label: m["home.settings"](), onClick: openSettings },
    });
    return;
  }
  const id = showToast({
    title: m["git.save.saving"](),
    message: pushOnly
      ? m["git.save.sendingSaves"]({
          saves: count(
            m["git.save.savesN.one"],
            m["git.save.savesN.other"],
            state.ahead ?? 0,
          ),
          target,
        })
      : (sent === 1
          ? m["git.save.recording.one"]
          : m["git.save.recording.other"])({ n: sent, message }),
    variant: "neutral",
    progress: { value: pushOnly ? 50 : 35, total: 100 },
  });
  const end = (o: Parameters<typeof showToast>[0]) => {
    dismissToast(id);
    showToast(o);
  };
  if (!pushOnly) {
    try {
      await git.commit(
        t,
        workspace,
        message,
        view.leftOut.map((e) => e.path),
      );
      announceGitMoved(workspace);
    } catch (e) {
      end({
        title: m["git.save.failed"](),
        message: explain(e),
        variant: "error",
        action: { label: m["git.save.retry"](), onClick: again },
      });
      return;
    }
    for (const e of view.leftOut)
      showToast({
        message: m["git.leftOut"]({ name: e.path, size: formatSize(e.size) }),
        variant: "warning",
      });
    if (!connected) {
      end({
        title: m["git.save.done"](),
        message: m["git.save.doneLocal"]({ message, files }),
        variant: "success",
      });
      return;
    }
    updateToast(id, {
      message: m["git.save.sending"]({ what: sending, target }),
      progress: { value: 75, total: 100 },
    });
  }
  try {
    await git.push(t, workspace);
    announceGitMoved(workspace);
    end({
      title: m["git.save.done"](),
      message: pushOnly
        ? m["git.save.donePushOnly"]({ target })
        : m["git.save.doneSent"]({ message, files, target }),
      variant: "success",
    });
  } catch (e) {
    announceGitMoved(workspace);
    if (e instanceof GitError && e.code === "behind")
      end({
        title: m["git.save.notSent"](),
        message: m["git.save.pullFirst"](),
        variant: "warning",
        action: { label: m["git.pull"](), onClick: hooks.onPull },
      });
    else
      end({
        title: m["git.save.notSent"](),
        message: explain(e),
        variant: "error",
        action: { label: m["git.save.retry"](), onClick: again },
      });
  }
}
