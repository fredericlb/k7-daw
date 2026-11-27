// ForgePicker.tsx — one account's repositories as a list to pick from: k7 workspaces first, a filter, a way to make one.
// What a reader gets wrong: it never signs in nor asks a permission; that is Settings → Services, before it is ever shown.

import { Button } from "@studio/ui/button";
import { Checkbox } from "@studio/ui/checkbox";
import { Lock, Plus } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { SectionHeading } from "@studio/ui/section-heading";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useId, useState } from "react";
import { m } from "../paraglide/messages.js";
import { droppedSignIns, isSignout } from "./forgeState";
import { explain, GitError, type GitTransport, git } from "./gitApi";
import {
  type Ago,
  arrange,
  type GithubRepo,
  pushedAgo,
  shortName,
} from "./githubState";

export interface PickedRepo {
  sshUrl: string;
  fullName: string;
}

const AGO = {
  now: () => m["git.gh.agoNow"](),
  minutes: (n: number) => m["git.gh.agoMinutes"]({ n }),
  hours: (n: number) => m["git.gh.agoHours"]({ n }),
  days: (n: number) => m["git.gh.agoDays"]({ n }),
  months: (n: number) => m["git.gh.agoMonths"]({ n }),
  years: (n: number) => m["git.gh.agoYears"]({ n }),
};

const agoText = ({ unit, n }: Ago): string => AGO[unit](n);

/** The account a picker lists: its id in the routes, and the name a refusal says. */
export interface PickerForge {
  id: string;
  name: string;
}

export const GITHUB: PickerForge = { id: "github.com", name: "GitHub" };

export function ForgePicker({
  t,
  forge = GITHUB,
  refresh,
  createName,
  disabled,
  onPick,
}: {
  t: GitTransport;
  forge?: PickerForge;
  refresh: () => Promise<void>;
  /** The workspace's name, which a new repository takes; absent where a repository cannot be made (Clone). */
  createName?: string;
  disabled: boolean;
  onPick: (r: PickedRepo) => void;
}) {
  const [repos, setRepos] = useState<GithubRepo[] | null>(null);
  const [error, setError] = useState("");
  const [filter, setFilter] = useState("");
  const [making, setMaking] = useState(false);

  useEffect(() => {
    let live = true;
    git
      .forgeRepos(t, forge.id)
      .then((a) => live && setRepos(a.repos))
      .catch((e) => {
        if (!live) return;
        if (e instanceof GitError && e.code === "scope")
          setError(m["git.gh.signInAgain"]());
        else if (e instanceof GitError && isSignout(e.code)) {
          droppedSignIns.add(forge.id);
          void refresh();
        } else setError(explain(e));
      });
    return () => {
      live = false;
    };
  }, [t, forge.id, refresh]);

  const shown = arrange(repos ?? [], filter);
  const now = Date.now();
  const section = (title: string, list: GithubRepo[]) =>
    list.length > 0 && (
      <section aria-label={title} className="flex flex-col gap-1">
        <SectionHeading className="px-1">{title}</SectionHeading>
        {list.map((r) => (
          <button
            key={r.fullName}
            type="button"
            disabled={disabled}
            onClick={() => onPick({ sshUrl: r.sshUrl, fullName: r.fullName })}
            className="flex items-center gap-2 rounded-xl px-3 py-2 text-left hover:bg-(--k7-row) disabled:opacity-40"
          >
            <span className="min-w-0 flex-1 truncate type-body">
              {r.fullName}
            </span>
            {r.private && (
              <span className="flex items-center gap-1 text-fg-3 type-status">
                <Lock className="size-3" />
                {m["git.gh.private"]()}
              </span>
            )}
            <span className="shrink-0 text-fg-3 type-status">
              {agoText(pushedAgo(r.pushedAt, now))}
            </span>
          </button>
        ))}
      </section>
    );
  return (
    <div className="flex flex-col gap-3">
      {making && createName !== undefined ? (
        <NewRepository
          t={t}
          forge={forge}
          initial={shortName(createName)}
          disabled={disabled}
          onMade={onPick}
          onCancel={() => setMaking(false)}
        />
      ) : (
        <>
          <div className="flex gap-2">
            <Input
              aria-label={m["git.gh.filter"]()}
              placeholder={m["git.gh.filter"]()}
              autoComplete="off"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
            />
            {createName !== undefined && (
              <Button
                type="button"
                variant="secondary"
                disabled={disabled}
                onClick={() => setMaking(true)}
              >
                <Plus className="mr-1 size-3" />
                {m["git.gh.new"]()}
              </Button>
            )}
          </div>
          {error && (
            <p role="alert" className="text-error type-secondary">
              {error}
            </p>
          )}
          <div className="flex max-h-72 flex-col gap-3 overflow-y-auto">
            {repos === null && !error && (
              <p className="text-fg-2 type-secondary" role="status">
                <Spinner size="xs" inherit className="mr-1 inline" />
                {m["git.gh.loading"]()}
              </p>
            )}
            {section(m["git.gh.k7Heading"](), shown.k7)}
            {section(m["git.gh.allHeading"](), shown.others)}
            {repos !== null && shown.k7.length + shown.others.length === 0 && (
              <p className="text-fg-2 type-secondary">{m["git.gh.none"]()}</p>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function NewRepository({
  t,
  forge,
  initial,
  disabled,
  onMade,
  onCancel,
}: {
  t: GitTransport;
  forge: PickerForge;
  initial: string;
  disabled: boolean;
  onMade: (r: PickedRepo) => void;
  onCancel: () => void;
}) {
  const [name, setName] = useState(initial);
  const [isPrivate, setPrivate] = useState(true);
  const privateId = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const make = async () => {
    setBusy(true);
    setError("");
    try {
      onMade(await git.forgeCreate(t, forge.id, name.trim(), isPrivate));
    } catch (e) {
      setError(
        e instanceof GitError && e.code === "taken"
          ? m["git.fj.taken"]({ name: name.trim(), server: forge.name })
          : explain(e),
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-3">
      <Input
        autoFocus
        aria-label={m["git.gh.newName"]()}
        placeholder={m["git.gh.newName"]()}
        value={name}
        onChange={(e) => setName(e.target.value)}
      />
      <div className="flex items-center gap-2">
        <Checkbox
          id={privateId}
          checked={isPrivate}
          onCheckedChange={(c) => setPrivate(c === true)}
        />
        <label htmlFor={privateId} className="type-secondary">
          {m["git.gh.newPrivate"]()}
        </label>
      </div>
      {error && (
        <p role="alert" className="text-error type-secondary">
          {error}
        </p>
      )}
      <div className="flex gap-2">
        <Button
          type="button"
          disabled={!name.trim() || busy || disabled}
          onClick={() => void make()}
        >
          {busy && <Spinner size="xs" inherit className="mr-1" />}
          {busy ? m["git.gh.creating"]() : m["git.gh.create"]()}
        </Button>
        <Button
          type="button"
          variant="ghost"
          disabled={busy}
          onClick={onCancel}
        >
          {m["common.cancel"]()}
        </Button>
      </div>
    </div>
  );
}
