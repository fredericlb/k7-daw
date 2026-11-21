// ConflictCard.tsx — the open conflict of a merge: where it is and one card per choice it offers (Mine, Theirs, ...).
// What a reader gets wrong: a card draws `offered`, so "both" never shows where the merge would refuse it.

import { cn } from "@studio/ui/lib/utils";
import { moveFocus } from "../menuTabs";
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

/** The text one side shows: its content, or "(removed)" where that side deleted it. */
function sideText(c: Conflict, side: "mine" | "theirs"): string {
  return c[side] ?? m["git.sideRemoved"]();
}

/** The radio's text: Keep mine, Keep theirs, Keep both, or the removed file's own two. */
const radioLabel = (c: Choice): string =>
  c === "mine"
    ? m["git.keepMine"]()
    : c === "theirs"
      ? m["git.keepTheirs"]()
      : c === "both"
        ? m["git.keepBoth"]()
        : choiceLabel(c);

function Side({
  title,
  source,
  text,
}: {
  title: string;
  source: string;
  text: string;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-1 rounded-2xl border border-(--k7-hairline) bg-(--k7-row) p-3">
      <span className="type-caps text-fg-3">{title}</span>
      <span className="type-status text-fg-3">{source}</span>
      <pre className="max-h-40 overflow-auto whitespace-pre-wrap break-all type-mono-value">
        {text}
      </pre>
    </div>
  );
}

export function ConflictCard({
  conflict: c,
  chosen,
  disabled,
  onPick,
  theirsFrom = "",
}: {
  conflict: Conflict;
  chosen: Choice | undefined;
  disabled: boolean;
  onPick: (choice: Choice) => void;
  /** Where their side comes from, as the remote and branch say it. */
  theirsFrom?: string;
}) {
  const mine = sideText(c, "mine");
  const theirs = sideText(c, "theirs");
  return (
    <div className="flex flex-col gap-3" data-conflict={c.id}>
      <div className="truncate type-mono-value">{whereOf(c)}</div>
      {c.kind === "binary" ? (
        <p className="type-secondary text-fg-2">{m["git.binaryBoth"]()}</p>
      ) : (
        <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
          <Side
            title={m["git.sideMineCaps"]()}
            source={m["git.sourceMine"]()}
            text={mine ?? ""}
          />
          <Side
            title={m["git.sideTheirsCaps"]()}
            source={theirsFrom}
            text={theirs ?? ""}
          />
        </div>
      )}
      <p className="type-caps text-fg-3">{m["git.choiceCaps"]()}</p>
      <div
        role="radiogroup"
        aria-label={m["git.choose"]()}
        className="flex flex-col gap-1.5"
        // One tab stop for the group; the arrows move the choice and the focus together, as a radio group does.
        onKeyDown={(e) => {
          const all = offered(c);
          const at = chosen ? all.indexOf(chosen) : -1;
          const key =
            e.key === "ArrowRight"
              ? "ArrowDown"
              : e.key === "ArrowLeft"
                ? "ArrowUp"
                : e.key;
          const to = disabled ? null : moveFocus(key, at, all.length);
          if (to === null) return;
          e.preventDefault();
          onPick(all[to]);
          e.currentTarget
            .querySelectorAll<HTMLElement>('[role="radio"]')
            [to]?.focus();
        }}
      >
        {offered(c).map((o, i) => (
          <button
            key={o}
            type="button"
            role="radio"
            aria-checked={chosen === o}
            tabIndex={chosen === o || (!chosen && i === 0) ? 0 : -1}
            disabled={disabled}
            onClick={() => onPick(o)}
            className={cn(
              "type-body flex min-h-11 cursor-pointer items-center gap-3 rounded-xl border px-3 text-left outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:opacity-40",
              chosen === o
                ? "border-primary bg-on-fill"
                : "border-(--k7-hairline-strong) bg-(--k7-row)",
            )}
          >
            <span
              aria-hidden
              className={cn(
                "size-4 shrink-0 rounded-full border",
                chosen === o
                  ? "border-primary bg-primary"
                  : "border-(--k7-hairline-strong)",
              )}
            />
            {radioLabel(o)}
          </button>
        ))}
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
