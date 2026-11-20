// ConflictCard.tsx — one conflict of a merge: where it is (a key path, a line, a file), the two sides, and the
// choices it offers. What a reader gets wrong: a card draws `offered`, so "both" never shows where it would be refused.

import { Button } from "@studio/ui/button";
import { m } from "../paraglide/messages.js";
import type { Choice, Conflict } from "./gitTypes";
import { offered } from "./mergeState";

const LABEL = {
  mine: "git.choiceMine",
  theirs: "git.choiceTheirs",
  both: "git.choiceBoth",
  keep: "git.choiceKeep",
  remove: "git.choiceRemove",
} as const satisfies Record<Choice, string>;

/** Where a conflict is, as a person reads it. */
function whereOf(c: Conflict): string {
  if (c.kind === "k7x" && c.keyPath) return c.keyPath;
  if (c.line !== undefined) return m["git.atLine"]({ line: c.line });
  return c.path;
}

function Side({ title, text }: { title: string; text: string | null }) {
  return (
    <div className="min-w-0 flex-1">
      <div className="text-[10px] text-muted-foreground">{title}</div>
      <pre className="max-h-32 overflow-auto whitespace-pre-wrap break-all rounded bg-muted/40 p-1 text-[11px]">
        {text ?? m["git.sideRemoved"]()}
      </pre>
    </div>
  );
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
    <li
      className="flex flex-col gap-2 rounded border px-3 py-2 text-xs"
      data-conflict={c.id}
    >
      <div className="truncate font-mono">{whereOf(c)}</div>
      {c.kind === "binary" ? (
        <p className="text-muted-foreground">{m["git.binaryBoth"]()}</p>
      ) : (
        <div className="flex gap-2">
          <Side title={m["git.sideMine"]()} text={c.mine} />
          <Side title={m["git.sideTheirs"]()} text={c.theirs} />
        </div>
      )}
      <fieldset className="flex flex-wrap gap-2" aria-label={m["git.choose"]()}>
        {offered(c).map((o) => (
          <Button
            key={o}
            size="xs"
            variant={chosen === o ? "default" : "secondary"}
            aria-pressed={chosen === o}
            disabled={disabled}
            onClick={() => onPick(o)}
          >
            {m[LABEL[o]]()}
          </Button>
        ))}
      </fieldset>
      {chosen === "both" && c.kind === "k7x" && (
        <p className="text-muted-foreground">{m["git.bothNamed"]()}</p>
      )}
      {chosen === "both" && c.kind === "binary" && (
        <p className="text-muted-foreground">{m["git.bothBeside"]()}</p>
      )}
    </li>
  );
}
