// ClipOpsMenu.tsx — the waveform's operations behind one lightning button, as the roll's: Trim, and Transformations,
// every chain the project and the libraries offer run over this clip. Trim opens its gate as a popover from this button.

import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Zap } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { ChainOpsPopover } from "./ChainOpsList";
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
  // The chains are a popover of their own, opened by the menu's second row: a pick must close it.
  const [opsOpen, setOpsOpen] = useState(false);
  const working = p.busy === "trim" || p.busy === "process";
  const button = (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="glass"
          size="icon-md"
          className="shrink-0"
          disabled={p.disabled}
          aria-label={m["clip.operations"]()}
          title={m["clip.operations"]()}
          aria-busy={working || undefined}
        >
          {working ? <Spinner size="sm" inherit /> : <Zap className="size-4" />}
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => p.onGateOpen(true)}>
          {m["clip.trim"]()}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => setOpsOpen(true)}>
          {m["clip.transformations"]()}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
  const withChains = (
    <ChainOpsPopover
      anchor={button}
      open={opsOpen}
      onOpenChange={setOpsOpen}
      search={p.opSearch}
      onSearch={p.onOpSearch}
      ops={p.ops}
      pending={p.opsPending}
      onPick={p.onPickOp}
    />
  );
  return (
    <TrimGate
      anchor={withChains}
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
