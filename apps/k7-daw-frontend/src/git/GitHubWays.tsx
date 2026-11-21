// GitHubWays.tsx — what Connect and Clone open on when the build offers GitHub: "Sign in with GitHub" or "Another host…".
// What a reader gets wrong: signed in, it goes straight to the picker, with "Another host…" still beside it.

import { Button } from "@studio/ui/button";
import { GitBranch } from "@studio/ui/icons";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { DeviceFlow } from "./DeviceFlow";
import { GitHubPicker, type PickedRepo } from "./GitHubPicker";
import type { GitTransport } from "./gitApi";
import { type GithubStatus, SIGN_IN_SCOPES, signedIn } from "./githubState";

export function GitHubWays({
  t,
  status,
  refresh,
  createName,
  disabled,
  onPick,
  onOther,
}: {
  t: GitTransport;
  status: GithubStatus;
  refresh: () => Promise<void>;
  createName?: string;
  disabled: boolean;
  onPick: (r: PickedRepo) => void;
  onOther: () => void;
}) {
  const [signing, setSigning] = useState(false);
  const [note, setNote] = useState("");
  const other = (
    <Button
      type="button"
      variant="ghost"
      className="self-start"
      onClick={onOther}
    >
      {m["git.gh.other"]()}
    </Button>
  );
  if (signedIn(status))
    return (
      <div className="flex flex-col gap-3">
        <p className="text-fg-2 type-secondary">
          {m["git.gh.signedAs"]({ login: status.login ?? "" })}
        </p>
        {note && <p className="text-fg-2 type-secondary">{note}</p>}
        <GitHubPicker
          t={t}
          status={status}
          refresh={refresh}
          createName={createName}
          disabled={disabled}
          onPick={onPick}
          onBack={onOther}
        />
        {other}
      </div>
    );
  if (signing)
    return (
      <DeviceFlow
        t={t}
        scopes={SIGN_IN_SCOPES}
        onDone={(_login, keyNote) => {
          setNote(keyNote ?? "");
          setSigning(false);
          void refresh();
        }}
        onCancel={() => setSigning(false)}
      />
    );
  return (
    <div className="flex flex-col gap-3">
      <p className="text-fg-2 type-secondary">{m["git.gh.intro"]()}</p>
      <Button
        type="button"
        className="self-start"
        onClick={() => setSigning(true)}
      >
        <GitBranch className="mr-1 size-4" />
        {m["git.gh.signIn"]()}
      </Button>
      {other}
    </div>
  );
}
