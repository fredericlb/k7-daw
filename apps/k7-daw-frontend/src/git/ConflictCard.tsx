// ConflictCard.tsx — the open conflict of a merge: where it is and one card per choice it offers (Mine, Theirs, ...).
// What a reader gets wrong: a card draws `offered`, so "both" never shows where the merge would refuse it.

import { cn } from "@studio/ui/lib/utils";
import { m } from "../paraglide/messages.js";
import type { Choice, Conflict } from "./gitTypes";
import { offered } from "./mergeState";

const LABEL = {
  mine: "git.sideMine",
  theirs: "git.sideTheirs",
  both: "git.sideBoth",
  keep: "git.choiceKeep",
  remove: "git.choiceRemove",
} as const satisfies Record<Choice, string>;

/** A choice as the screen names it. */
export const choiceLabel = (c: Choice): string => m[LABEL[c]]();

/** Where a conflict is, as a person reads it. */
export function whereOf(c: Conflict): string {
  if (c.kind === "k7x" && c.keyPath) return c.keyPath;
  if (c.line !== undefined) return m["git.atLine"]({ line: c.line });
  return c.path;
}

/** The text a side card shows, or null where a choice has no side of its own. */
function sideText(c: Conflict, choice: Choice): string | null | undefined {
  if (c.kind === "binary") return undefined;
  if (choice === "mine") return c.mine ?? m["git.sideRemoved"]();
  if (choice === "theirs") return c.theirs ?? m["git.sideRemoved"]();
  return undefined;
}

export function ConflictCard({
  conflict: c,
  chosen,
  disabled,
  onPick,
}: {
  conflict: Conflict;
  chosen: Choice | undefined;
  disabled: boolean;
  onPick: (choice: Choice) => void;
}) {
  return (
    <div className="flex flex-col gap-2" data-conflict={c.id}>
      <div className="truncate type-mono-value">{whereOf(c)}</div>
      {c.kind === "binary" && (
        <p className="type-secondary text-fg-2">{m["git.binaryBoth"]()}</p>
      )}
      <div
        role="radiogroup"
        aria-label={m["git.choose"]()}
        className="grid grid-cols-1 gap-2 sm:grid-cols-2"
      >
        {offered(c).map((o) => {
          const text = sideText(c, o);
          return (
            <button
              key={o}
              type="button"
              role="radio"
              aria-checked={chosen === o}
              aria-label={choiceLabel(o)}
              disabled={disabled}
              onClick={() => onPick(o)}
              className={cn(
                "flex min-w-0 cursor-pointer flex-col gap-1 rounded-2xl border p-3 text-left outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:opacity-40",
                chosen === o
                  ? "border-primary bg-on-fill"
                  : "border-(--k7-hairline-strong) bg-(--k7-row)",
              )}
            >
              <span className="type-label">{choiceLabel(o)}</span>
              {text !== undefined && (
                <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all type-mono-value">
                  {text}
                </pre>
              )}
            </button>
          );
        })}
      </div>
      {chosen === "both" && c.kind === "k7x" && (
        <p className="type-secondary text-fg-2">{m["git.bothNamed"]()}</p>
      )}
      {chosen === "both" && c.kind === "binary" && (
        <p className="type-secondary text-fg-2">{m["git.bothBeside"]()}</p>
      )}
    </div>
  );
}
