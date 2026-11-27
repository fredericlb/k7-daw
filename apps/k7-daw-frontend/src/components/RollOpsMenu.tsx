// RollOpsMenu.tsx — the piano roll's operations, behind one lightning button in the title bar: today, the length times two.
// "Times two" is `sequence double`, which plays the notes twice over the doubled length (docs/clipEditors.md).

import { Button } from "@studio/ui/button";
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
        <Button
          variant="glass"
          size="icon-md"
          className="shrink-0"
          aria-label={m["roll.ops"]()}
          title={m["roll.ops"]()}
        >
          <Zap className="size-4" />
        </Button>
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
