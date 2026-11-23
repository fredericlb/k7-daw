// serviceTabs.tsx — the three tabs of the git pages: GitHub, Forgejo (one picker per signed-in account) and Other host.
// What a reader gets wrong: GitHub is never signed in here (Settings → Services); a Forgejo server may be added and signed in.

import { Button } from "@studio/ui/button";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { m } from "../paraglide/messages.js";
import { SETTINGS_ROUTE } from "../projects/api";
import { ForgejoRows } from "./ForgejoSection";
import { ForgePicker, type PickedRepo } from "./ForgePicker";
import { accountLabel, canPickFrom, type ForgeAccount } from "./forgeState";
import type { ServiceTab } from "./GitPage";
import type { GitTransport } from "./gitApi";
import type { GitPageTab } from "./gitPageTabs";

function Pickers({
  t,
  accounts,
  refresh,
  createName,
  disabled,
  onPick,
}: {
  t: GitTransport;
  accounts: ForgeAccount[];
  refresh: () => Promise<void>;
  createName?: string;
  disabled: boolean;
  onPick: (r: PickedRepo) => void;
}) {
  return (
    <>
      {accounts.map((a) => (
        <section
          key={a.id}
          aria-label={accountLabel(a)}
          className="flex flex-col gap-3"
        >
          <p className="type-secondary text-fg-2">
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
    </>
  );
}

/** GitHub with no account that lists repositories: the sign-in is in Settings → Services. */
function GitHubElsewhere({ available }: { available: boolean }) {
  const navigate = useNavigate();
  if (!available)
    return (
      <p className="type-secondary text-fg-2">{m["git.gh.unavailable"]()}</p>
    );
  return (
    <div className="flex flex-col items-start gap-3">
      <p className="type-secondary text-fg-2">
        {m["git.page.githubInSettings"]()}
      </p>
      <Button
        variant="secondary"
        onClick={() => navigate(`${SETTINGS_ROUTE}?tab=services`)}
      >
        {m["git.page.openServices"]()}
      </Button>
    </div>
  );
}

export function serviceTabs({
  t,
  accounts,
  refresh,
  createName,
  disabled,
  onPick,
  other,
}: {
  t: GitTransport | null;
  /** Every forge account the device knows, signed in or not; null while it is read. */
  accounts: ForgeAccount[] | null;
  refresh: () => Promise<void>;
  createName?: string;
  disabled: boolean;
  onPick: (r: PickedRepo) => void;
  /** The Other host tab's form. */
  other: ReactNode;
}): ServiceTab[] {
  const all = accounts ?? [];
  const github = all.filter((a) => a.kind === "github");
  const forgejo = all.filter((a) => a.kind === "forgejo");
  const pickGithub = github.filter(canPickFrom);
  const pickForgejo = forgejo.filter(canPickFrom);
  const picker = (list: ForgeAccount[]) =>
    t ? (
      <Pickers
        t={t}
        accounts={list}
        refresh={refresh}
        createName={createName}
        disabled={disabled}
        onPick={onPick}
      />
    ) : null;
  const tabs: Record<GitPageTab, ServiceTab> = {
    github: {
      key: "github",
      label: m["git.gh.account"](),
      notSetUp: pickGithub.length === 0,
      content:
        pickGithub.length > 0 ? (
          picker(pickGithub)
        ) : (
          <GitHubElsewhere available={github.some((a) => a.available)} />
        ),
    },
    forgejo: {
      key: "forgejo",
      label: m["git.page.forgejo"](),
      notSetUp: pickForgejo.length === 0,
      content:
        pickForgejo.length > 0 ? (
          picker(pickForgejo)
        ) : t ? (
          <ForgejoRows t={t} accounts={forgejo} refresh={refresh} />
        ) : null,
    },
    other: {
      key: "other",
      label: m["git.page.other"](),
      content: other,
    },
  };
  return [tabs.github, tabs.forgejo, tabs.other];
}
