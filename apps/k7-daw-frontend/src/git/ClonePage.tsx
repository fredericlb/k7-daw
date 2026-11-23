// ClonePage.tsx — Clone a repository: brings one onto this device as a new workspace, picked from a forge account or typed.
// The name is the folder, so it is checked as any workspace's is before the call; it follows the address until typed.

import { Input } from "@studio/ui/input";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { workspaceFolderName } from "../projects/projectName";
import type { PickedRepo } from "./ForgePicker";
import { useGitTransport } from "./GitContext";
import { GitPage } from "./GitPage";
import { git, type Remote } from "./gitApi";
import { shortName } from "./githubState";
import { takeClonePrefill } from "./gitPages";
import { firstTab, type GitPageTab, nameFromUrl } from "./gitPageTabs";
import { OtherHostForm } from "./OtherHostForm";
import { serviceTabs } from "./serviceTabs";
import { useForges } from "./useForges";
import { useRemoteCall } from "./useRemoteCall";

export function ClonePage({
  onClose,
  onCloned,
}: {
  onClose: () => void;
  onCloned: () => void;
}) {
  const t = useGitTransport();
  const forges = useForges(t);
  const [prefill] = useState(takeClonePrefill);
  // The name typed or given; null while it follows the address.
  const [typed, setTyped] = useState<string | null>(prefill?.name ?? null);
  const [fromPick, setFromPick] = useState(false);

  const clone = async (r: Remote) => {
    if (!t || !folder) return;
    await git.clone(t, folder, r);
    prefill?.onCloned?.();
    onCloned();
    onClose();
  };
  const call = useRemoteCall(clone, prefill?.url ?? "");
  const name = typed ?? nameFromUrl(call.form.url);
  const { folder, error: badName } = workspaceFolderName(name);
  const [tab, setTab] = useState<GitPageTab | null>(null);
  const shown =
    tab ??
    (forges.accounts !== null
      ? firstTab(forges.accounts, prefill?.url ?? "")
      : "other");

  // A pick only fills the address and the name: the person presses Clone (docs/git.md).
  const pick = (r: PickedRepo) => {
    call.dispatch({ type: "edit", value: r.sshUrl });
    setTyped(shortName(r.fullName));
    setFromPick(true);
    setTab("other");
  };

  return (
    <GitPage
      title={m["git.cloneTitle"]()}
      region={m["git.cloneTitle"]()}
      intro={m["git.cloneBody"]()}
      tab={shown}
      onTab={(k) => setTab(k as GitPageTab)}
      busy={call.busy}
      onBack={onClose}
      tabs={serviceTabs({
        t,
        accounts: forges.accounts,
        refresh: forges.refresh,
        disabled: call.busy || !t,
        onPick: pick,
        other: (
          <OtherHostForm
            call={{
              ...call,
              dispatch: (e) => {
                if (e.type === "edit") setFromPick(false);
                call.dispatch(e);
              },
            }}
            submit={m["git.cloneSubmit"]()}
            step={m["git.cloning"]()}
            canSubmit={Boolean(folder) && t !== null}
            hideKey={fromPick}
            before={
              <div className="flex flex-col gap-1.5">
                <Input
                  aria-label={m["git.cloneName"]()}
                  placeholder={m["git.cloneName"]()}
                  value={name}
                  disabled={call.busy}
                  onChange={(e) => setTyped(e.target.value)}
                />
                {badName && name.trim() && (
                  <p className="text-error type-secondary">{badName}</p>
                )}
              </div>
            }
          />
        ),
      })}
    />
  );
}
