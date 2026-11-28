// SyncSettingsPage.tsx — a workspace's Sync settings: connect it to a repository picked from a forge account or typed.
// A host without large-file storage is said after the connection, not refused: the push is what it refuses.

import { askNotice } from "@studio/project-shell/ask";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import type { PickedRepo } from "./ForgePicker";
import { useGitTransport } from "./GitContext";
import { GitPage } from "./GitPage";
import { git, type Remote } from "./gitApi";
import { announceGitMoved } from "./gitBus";
import { firstTab, type GitPageTab } from "./gitPageTabs";
import { remoteName } from "./gitState";
import { OtherHostForm } from "./OtherHostForm";
import { serviceTabs } from "./serviceTabs";
import { useForges } from "./useForges";
import { useRemoteCall } from "./useRemoteCall";

export function SyncSettingsPage({
  workspace,
  onClose,
  onConnected,
}: {
  workspace: string;
  onClose: () => void;
  onConnected: () => void;
}) {
  const t = useGitTransport();
  const forges = useForges(t);
  // The remote the workspace has now, which the ssh form shows; "" once read and none.
  const [current, setCurrent] = useState<string | null>(null);
  useEffect(() => {
    if (!t) return;
    let live = true;
    git
      .state(t, workspace)
      .then(
        (s) =>
          live && setCurrent(s.mode === "connected" ? (s.remote ?? "") : ""),
      )
      .catch(() => live && setCurrent(""));
    return () => {
      live = false;
    };
  }, [t, workspace]);

  const connect = async (r: Remote) => {
    if (!t) return;
    const done = await git.connect(t, workspace, r);
    announceGitMoved(workspace);
    onConnected();
    const remote = remoteName(done.remote || r.url);
    onClose();
    if (done.lfsWarning)
      void askNotice({
        title: m["git.noLfsTitle"](),
        body: m["git.noLfsBody"]({ host: remote }),
        confirmLabel: m["common.close"](),
      });
  };
  const call = useRemoteCall(connect, current ?? "");
  const [tab, setTab] = useState<GitPageTab | null>(null);
  const known = forges.accounts !== null && current !== null;
  const shown =
    tab ?? (known ? firstTab(forges.accounts ?? [], current ?? "") : "other");

  // A pick connects at once; a refusal moves to the ssh form, where the host-key question is asked.
  const pick = (r: PickedRepo) =>
    void call.go({ url: r.sshUrl }).then((ok) => {
      if (!ok) setTab("other");
    });

  return (
    <GitPage
      title={m["git.page.syncTitle"]()}
      subtitle={workspace}
      region={m["git.page.syncRegion"]({ workspace })}
      intro={m["git.connectBody"]()}
      tab={shown}
      onTab={(k) => setTab(k as GitPageTab)}
      busy={call.busy}
      onBack={onClose}
      tabs={serviceTabs({
        t,
        accounts: forges.accounts,
        refresh: forges.refresh,
        createName: workspace,
        disabled: call.busy || !t,
        onPick: pick,
        other: (
          <OtherHostForm
            call={call}
            submit={m["git.connectSubmit"]()}
            step={m["git.connecting"]()}
            canSubmit={t !== null}
          />
        ),
      })}
    />
  );
}
