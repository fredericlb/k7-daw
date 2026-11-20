// Home.tsx — what k7 draws when its address names NO project: the account's workspaces and the
// projects in them, the libraries. It holds the async — identity, the file
// service, and the two polls — for the presentational body beside it. The list is the SERVER's
// files and needs no engine. See ../../docs/projects.md.

import type { AccountLibrary, AccountWorkspace } from "@studio/web-protocol";
import { useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  homeAccountFiles,
  prefetchHome,
  takePrefetchedWorkspaces,
} from "../settings/accountFiles";
import { pollWhileVisible } from "../visiblePoll";
import { accountFiles } from "./api";
import { useHomeCommandsChanged } from "./HomeCommandBar";
import { HomeShell } from "./HomeFrame";
import { WS_PARAM } from "./homeCommands";
import { ProjectsPane } from "./ProjectsPane";
import type { AccountFiles } from "./workspaceOps";

/** The médiathèque's own: brisk enough that a change from another replica shows, slow enough to cost nothing on a ten-row list. */
const POLL_MS = 3000;

/** How often the page asks again for a file service that has not answered yet, and how many times — docs/projects.md. */
const ASK_AGAIN_MS = 1500;
const ASK_AGAIN_TIMES = 10;

// As this chunk arrives, which appMain starts beside the sign-in check: the list is then asked once the hello answers, not once the page mounts.
prefetchHome();

/** The device's home: the workspaces on this machine, its libraries and tasks. */
export function Home() {
  const [params, setParams] = useSearchParams();
  const openWs = params.get(WS_PARAM);

  const [workspaces, setWorkspaces] = useState<AccountWorkspace[]>([]);
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // The account's file service, or null; `undefined` until asked — "none" is a refusal we draw, "looking" is not.
  const [fileService, setFileService] = useState<
    AccountFiles | null | undefined
  >(undefined);

  /** The account's files on the server, engine or none — and no `hold()`: a claim is a lease ON A PROJECT, which this page has none of. */
  useEffect(() => {
    let live = true;
    let asked = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    // On a phone the app's file service boots seconds AFTER this page, so its first silence is not an answer.
    const ask = () => {
      void homeAccountFiles().then((f) => {
        if (!live) return;
        if (f === null && asked < ASK_AGAIN_TIMES) {
          asked += 1;
          timer = setTimeout(ask, ASK_AGAIN_MS);
          return;
        }
        setFileService(f);
      });
    };
    ask();
    return () => {
      live = false;
      if (timer) clearTimeout(timer);
    };
  }, []);

  // The libraries, read here for the notice and the workspace buttons; they are edited on their own page.
  const [libraries, setLibraries] = useState<AccountLibrary[] | undefined>(
    undefined,
  );
  useEffect(() => {
    if (fileService === undefined) return;
    if (fileService === null) return setLibraries([]);
    void accountFiles
      .libraries(fileService)
      .then((r) => setLibraries(r.libraries))
      .catch(() => setLibraries([]));
  }, [fileService]);

  /** The project list is the file service's answer, over the account's workspaces on the server. */
  // What the last poll answered, as text: an unchanged list keeps its identity, so no row re-renders.
  const lastWorkspaces = useRef("");
  const loadWorkspaces = useCallback(async (m: AccountFiles) => {
    try {
      const next = await (takePrefetchedWorkspaces(m) ??
        accountFiles.workspaces(m));
      const text = JSON.stringify(next);
      if (text !== lastWorkspaces.current) {
        lastWorkspaces.current = text;
        setWorkspaces(next);
      }
      setError(null);
    } catch (e) {
      // The list IS the screen's subject: say it failed rather than draw an empty one.
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    if (fileService === undefined) return; // still probing: neither a list nor a refusal
    if (fileService === null) {
      setWorkspaces([]);
      setBusy(false);
      return;
    }
    let stop = false;
    const tick = async () => {
      if (!stop) await loadWorkspaces(fileService);
    };
    void tick();
    const stopPoll = pollWhileVisible(tick, POLL_MS);
    return () => {
      stop = true;
      stopPoll();
    };
  }, [fileService, loadWorkspaces]);

  /** Reload NOW after a write from here, or a just-created project would take three seconds to appear. */
  const reread = useCallback(() => {
    if (fileService) void loadWorkspaces(fileService);
  }, [fileService, loadWorkspaces]);

  // A command from the bar wrote to the workspaces: the list is read at once, not at the next poll.
  useHomeCommandsChanged(reread);

  /** Unfold a workspace, or go back to the list — `replace`, browsing three of them not being worth three back-button entries. */
  const open = useCallback(
    (name: string | null) => {
      setParams(
        (p) => {
          const next = new URLSearchParams(p);
          if (name) next.set(WS_PARAM, name);
          else next.delete(WS_PARAM);
          return next;
        },
        { replace: true },
      );
    },
    [setParams],
  );

  return (
    <HomeShell>
      <div className="flex min-h-0 min-w-0 flex-1 flex-col overflow-y-auto">
        {error && (
          <code className="mx-6 mt-3 overflow-x-auto rounded bg-muted px-2 py-1 text-xs text-red-400">
            {error}
          </code>
        )}

        <ProjectsPane
          workspaces={workspaces}
          busy={busy}
          openWs={openWs}
          fileService={fileService ?? null}
          lookingForFiles={fileService === undefined}
          onOpen={open}
          onChanged={reread}
          libraries={libraries}
        />
      </div>
    </HomeShell>
  );
}
