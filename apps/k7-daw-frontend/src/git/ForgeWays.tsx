// ForgeWays.tsx — what Connect and Clone open on for a person signed in to a forge: one picker per account, "Another host…".
// What a reader gets wrong: nobody signs in here; the dialog shows this only after Settings → Services (`canPickFrom`).

import { Button } from "@studio/ui/button";
import { m } from "../paraglide/messages.js";
import { ForgePicker, type PickedRepo } from "./ForgePicker";
import { accountLabel, type ForgeAccount } from "./forgeState";
import type { GitTransport } from "./gitApi";

export function ForgeWays({
  t,
  accounts,
  refresh,
  createName,
  disabled,
  onPick,
  onOther,
}: {
  t: GitTransport;
  /** The accounts that can list repositories, in the order shown. */
  accounts: ForgeAccount[];
  refresh: () => Promise<void>;
  createName?: string;
  disabled: boolean;
  onPick: (r: PickedRepo) => void;
  onOther: () => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      {accounts.map((a) => (
        <section
          key={a.id}
          aria-label={accountLabel(a)}
          className="flex flex-col gap-3"
        >
          <p className="text-fg-2 type-secondary">
            {a.kind === "github"
              ? m["git.gh.signedAs"]({ login: a.login ?? "" })
              : m["git.fj.signedOn"]({
                  login: a.login ?? "",
                  server: accountLabel(a),
                })}
          </p>
          <ForgePicker
            t={t}
            forge={{ id: a.id, name: accountLabel(a) }}
            refresh={refresh}
            createName={createName}
            disabled={disabled}
            onPick={onPick}
          />
        </section>
      ))}
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
