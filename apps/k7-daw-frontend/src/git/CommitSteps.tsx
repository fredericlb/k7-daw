// CommitSteps.tsx — the progress list of Commit & Push: a Commit row and a Push row, each with its mark and its detail.
// What a reader gets wrong: a warn step (pull first) is not a failure, so it has no Try again.

import { Check, Loader2, TriangleAlert } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import type { StepRow } from "./commitState";

/** What each row reads by its state, so a finished step does not say it is still running. */
const LABEL = {
  commit: {
    pending: "git.stepCommitWaiting",
    active: "git.stepCommitting",
    done: "git.stepCommitted",
    warn: "git.stepCommitted",
    error: "git.stepCommitFailed",
  },
  push: {
    pending: "git.stepPushWaiting",
    active: "git.stepPushing",
    done: "git.stepPushed",
    warn: "git.stepPushWaiting",
    error: "git.stepPushFailed",
  },
} as const;

const MARK = {
  pending: "git.markPending",
  active: "git.markActive",
  done: "git.stepDone",
  warn: "git.markWarn",
  error: "git.markError",
} as const;

function Mark({ state }: { state: StepRow["state"] }) {
  const name = m[MARK[state]]();
  if (state === "done")
    return <Check role="img" aria-label={name} className="size-4 text-ok" />;
  if (state === "active")
    return (
      <Loader2 role="img" aria-label={name} className="size-4 animate-spin" />
    );
  if (state === "warn")
    return (
      <TriangleAlert role="img" aria-label={name} className="size-4 text-warn" />
    );
  if (state === "error")
    return (
      <TriangleAlert
        role="img"
        aria-label={name}
        className="size-4 text-error"
      />
    );
  return (
    <span
      role="img"
      aria-label={name}
      className="size-4 rounded-full border border-(--k7-hairline-strong)"
    />
  );
}

export function CommitSteps({
  rows,
  details,
}: {
  rows: StepRow[];
  /** What each row says under its name: the message and file count, the remote, the live sending line. */
  details: Partial<Record<StepRow["id"], string>>;
}) {
  return (
    <ul aria-label={m["git.progress"]()} className="flex flex-col gap-2">
      {rows.map((r) => (
        <li
          key={r.id}
          data-state={r.state}
          data-testid={r.state === "warn" ? "git-pull-first" : undefined}
          className="flex items-start gap-2 type-secondary"
        >
          <span className="mt-0.5">
            <Mark state={r.state} />
          </span>
          <span className="flex min-w-0 flex-col">
            <span className={r.state === "pending" ? "text-fg-2" : undefined}>
              {m[LABEL[r.id][r.state]]()}
            </span>
            {details[r.id] && r.state !== "warn" && r.state !== "error" && (
              <span className="text-fg-3">{details[r.id]}</span>
            )}
            {r.state === "warn" && (
              <span className="text-warn">{m["git.pullFirst"]()}</span>
            )}
            {r.state === "error" && r.detail && (
              <span role="alert" className="whitespace-pre-wrap text-error">
                {r.detail}
              </span>
            )}
          </span>
        </li>
      ))}
    </ul>
  );
}
