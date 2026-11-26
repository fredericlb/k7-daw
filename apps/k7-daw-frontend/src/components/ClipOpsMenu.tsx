// ClipOpsMenu.tsx — the waveform's operations behind one lightning button, as the roll's: Trim, and Transformations,
// every chain the project and the libraries offer run over this clip. Trim opens its gate as a popover from this button.

import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Loader2, Zap } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { ChainOpsList } from "./ChainOpsList";
import { TrimGate } from "./ClipGateControls";

export function ClipOpsMenu(p: {
  busy: string | null;
  disabled: boolean;
  gateOpen: boolean;
  onGateOpen: (open: boolean) => void;
  gate: number;
  onGate: (db: number) => void;
  keep: { from: number; to: number } | null;
  canTrim: boolean;
  onTrim: () => void;
  opSearch: string;
  onOpSearch: (s: string) => void;
  ops: string[];
  opsPending: boolean;
  onPickOp: (chain: string) => void;
}) {
  const working = p.busy === "trim" || p.busy === "process";
  const button = (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          disabled={p.disabled}
          aria-label={m["clip.operations"]()}
          title={m["clip.operations"]()}
          aria-busy={working || undefined}
          className="glass grid size-9 shrink-0 cursor-pointer place-items-center rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring) disabled:opacity-40"
        >
          {working ? (
            <Loader2 className="size-4 animate-spin" />
          ) : (
            <Zap className="size-4" />
          )}
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => p.onGateOpen(true)}>
          {m["clip.trim"]()}
        </DropdownMenuItem>
        <DropdownMenuSub>
          <DropdownMenuSubTrigger>
            {m["clip.transformations"]()}
          </DropdownMenuSubTrigger>
          <DropdownMenuSubContent className="w-64 p-1">
            <ChainOpsList
              search={p.opSearch}
              onSearch={p.onOpSearch}
              ops={p.ops}
              pending={p.opsPending}
              onPick={p.onPickOp}
            />
          </DropdownMenuSubContent>
        </DropdownMenuSub>
      </DropdownMenuContent>
    </DropdownMenu>
  );
  return (
    <TrimGate
      anchor={button}
      open={p.gateOpen}
      onOpenChange={p.onGateOpen}
      gate={p.gate}
      onGate={p.onGate}
      keep={p.keep}
      canTrim={p.canTrim}
      onTrim={p.onTrim}
    />
  );
}
