// RollOpsMenu.tsx — the piano roll's operations, behind one lightning button in the title bar: today, the length times two.
// "Times two" is `sequence double`, which plays the notes twice over the doubled length (docs/clipEditors.md).

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Zap } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";

export function RollOpsMenu({
  seqId,
  command,
}: {
  seqId: string;
  /** Absent ⇒ nothing is drawn: a roll that cannot be edited offers no operation. */
  command?: (line: string) => unknown;
}) {
  if (!command) return null;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          aria-label={m["roll.ops"]()}
          title={m["roll.ops"]()}
          className="glass grid size-9 shrink-0 cursor-pointer place-items-center rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring)"
        >
          <Zap className="size-4" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem
          title={m["seq.doubleHint"]()}
          onSelect={() => void command(`sequence double ${seqId}`)}
        >
          {m["roll.opDouble"]()}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
