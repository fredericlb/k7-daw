// MergeList.tsx — the left column of the Conflict screen: a count heading, one row per conflict with its file-kind glyph, the bulk choices.
// What a reader gets wrong: Mine for all and Theirs for all pass over binary and removed files (mergeState.ts).

import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { File, FileCode, FileText, FileX } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { moveFocus } from "../menuTabs";
import { m } from "../paraglide/messages.js";
import { choiceLabel, whereOf } from "./ConflictCard";
import type { Choice, Conflict } from "./gitTypes";

const KIND = {
  k7x: { Glyph: FileCode, name: "git.kindK7x" },
  text: { Glyph: FileText, name: "git.kindText" },
  binary: { Glyph: File, name: "git.kindBinary" },
  presence: { Glyph: FileX, name: "git.kindPresence" },
} as const;

export function MergeList({
  conflicts,
  chosen,
  currentId,
  disabled,
  onOpen,
  onAll,
}: {
  conflicts: Conflict[];
  chosen: Record<string, Choice>;
  currentId: string | undefined;
  disabled: boolean;
  onOpen: (id: string) => void;
  onAll: (side: "mine" | "theirs") => void;
}) {
  const n = conflicts.length;
  return (
    <div className="flex min-h-0 flex-col gap-2">
      <h2 className="type-caps px-1 text-fg-3">
        {(n === 1
          ? m["git.conflictsHeading.one"]
          : m["git.conflictsHeading.other"])({
          n,
        })}
      </h2>
      <div
        role="listbox"
        aria-label={m["git.conflictList"]()}
        className="flex min-h-0 flex-col gap-1 overflow-y-auto"
        // One tab stop for the list; the arrows, Home and End move the open conflict and the focus with it.
        onKeyDown={(e) => {
          const at = conflicts.findIndex((c) => c.id === currentId);
          const to = moveFocus(e.key, at, n);
          if (to === null) return;
          e.preventDefault();
          onOpen(conflicts[to].id);
          e.currentTarget
            .querySelectorAll<HTMLElement>('[role="option"]')
            [to]?.focus();
        }}
      >
        {conflicts.map((c) => {
          const { Glyph, name } = KIND[c.kind];
          return (
            <button
              key={c.id}
              type="button"
              role="option"
              aria-selected={c.id === currentId}
              tabIndex={c.id === currentId ? 0 : -1}
              onClick={() => onOpen(c.id)}
              className={cn(
                "flex cursor-pointer items-center gap-2 rounded-xl px-3 py-2 text-left outline-none focus-visible:shadow-(--k7-focus-ring)",
                c.id === currentId ? "bg-on-fill" : "hover:bg-on-fill",
              )}
            >
              <Glyph
                role="img"
                aria-label={m[name]()}
                className="size-4 shrink-0 text-fg-3"
              />
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate type-mono-value">{whereOf(c)}</span>
                <span className="truncate type-secondary text-fg-2">
                  {c.path}
                </span>
              </span>
              <Chip variant="status" tone={chosen[c.id] ? "renamed" : "choose"}>
                {chosen[c.id] ? choiceLabel(chosen[c.id]) : m["git.toChoose"]()}
              </Chip>
            </button>
          );
        })}
      </div>
      {n > 1 && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            {(["mine", "theirs"] as const).map((side) => (
              <Button
                key={side}
                size="xs"
                variant="ghost"
                disabled={disabled}
                onClick={() => onAll(side)}
              >
                {m[side === "mine" ? "git.allMine" : "git.allTheirs"]()}
              </Button>
            ))}
          </div>
          <p className="type-status px-1 text-fg-3">{m["git.bulkNote"]()}</p>
        </>
      )}
    </div>
  );
}
