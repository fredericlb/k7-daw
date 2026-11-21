// GitHubWays.tsx — what Connect and Clone open on for a person signed in with GitHub: the picker, and "Another host…".
// What a reader gets wrong: nobody signs in here; the dialog shows this only after Settings → Services (`canPick`).

import { Button } from "@studio/ui/button";
import { m } from "../paraglide/messages.js";
import { GitHubPicker, type PickedRepo } from "./GitHubPicker";
import type { GitTransport } from "./gitApi";
import type { GithubStatus } from "./githubState";

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
  return (
    <div className="flex flex-col gap-3">
      <p className="text-fg-2 type-secondary">
        {m["git.gh.signedAs"]({ login: status.login ?? "" })}
      </p>
      <GitHubPicker
        t={t}
        refresh={refresh}
        createName={createName}
        disabled={disabled}
        onPick={onPick}
      />
      <Button
        type="button"
        variant="ghost"
        className="self-start"
        onClick={onOther}
      >
        {m["git.gh.other"]()}
      </Button>
    </div>
  );
}
