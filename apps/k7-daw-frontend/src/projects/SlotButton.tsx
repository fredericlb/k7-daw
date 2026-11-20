// SlotButton.tsx — a frozen chain's "save in a slot" button: straight into this machine's engine when it is the only
// target, or a menu of it and the engines this account owns elsewhere, saved there through their server.
// See ../../docs/slots.md.

import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Cpu } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import type { PublicEngine } from "../types";

/** Where a slot goes: this machine's own engine, or one owned elsewhere. */
export type SlotTarget =
  | { kind: "here" }
  | { kind: "remote"; engine: PublicEngine };

export function SlotButton({
  here,
  remote,
  refusal,
  onSave,
}: {
  /** This machine runs an engine. */
  here: boolean;
  remote: PublicEngine[];
  /** Why the chain cannot play live, which no slot can either. */
  refusal?: string | null;
  onSave: (t: SlotTarget) => void;
}) {
  if (!here && !remote.length) return null;
  const title = refusal
    ? m["frozen.notLive"]({ why: refusal })
    : m["frozen.saveSlot"]();
  const button = (onClick?: () => void) => (
    <Button
      size="icon-sm"
      variant="ghost"
      disabled={!!refusal}
      onClick={onClick}
      aria-label={title}
      title={title}
    >
      <Cpu className="size-5" />
    </Button>
  );
  if (!remote.length) return button(() => onSave({ kind: "here" }));
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>{button()}</DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-56">
        {here && (
          <DropdownMenuItem
            className="text-sm"
            onSelect={() => onSave({ kind: "here" })}
          >
            {m["frozen.slotHere"]()}
          </DropdownMenuItem>
        )}
        {remote.map((e) => (
          <DropdownMenuItem
            key={e.id}
            className="text-sm"
            onSelect={() => onSave({ kind: "remote", engine: e })}
          >
            {m["frozen.slotOn"]({ host: e.hostname })}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
