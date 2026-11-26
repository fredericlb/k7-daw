// ClipControls.tsx — the audio clip editor's options: the event's gain, Loop / Trim / Operations (and Discard and
// Done for a take just filed). They are the TITLE BAR's, like every editor's (docs/editors.md); a phone keeps them in the body.

import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { DockHeaderRow } from "@studio/ui/editor-dock";
import { Check, Loader2, Trash2 } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { ChainOpsList } from "./ChainOpsList";
import { OnsetThreshold, TrimGate } from "./ClipGateControls";
import { EventGainSlider } from "./EventGainSlider";
import { LoopToggle } from "./LoopBrace";

export interface ClipControlsProps {
  gainDb: number;
  /** The event the loop and gain verbs address; null when nothing places the clip. */
  loopAddr: string | null;
  looped: boolean;
  command: (line: string) => unknown;
  clip: string | null;
  busy: string | null;
  gateOpen: boolean;
  onGateOpen: (open: boolean) => void;
  gate: number;
  onGate: (db: number) => void;
  keep: { from: number; to: number } | null;
  canTrim: boolean;
  onTrim: () => void;
  /** The onset threshold shows only under the Split tool. */
  editingOnsets: boolean;
  sensitivity: number | undefined;
  onSensitivity: (v: number) => void;
  opSearch: string;
  onOpSearch: (v: string) => void;
  ops: string[];
  opsPending: boolean;
  onPickOp: (chain: string) => void;
  onDiscard?: () => void;
  /** Only for a take just filed: Done files it as reviewed, which the dock's own Close does not. */
  onDone?: () => void;
}

export function ClipControls(p: ClipControlsProps) {
  return (
    <DockHeaderRow bodyOnPhone>
      <div className="flex shrink-0 flex-wrap items-center gap-2 text-xs text-muted-foreground">
        <EventGainSlider
          db={p.gainDb}
          onCommit={
            p.loopAddr
              ? (db) =>
                  void p.command(`event gain ${p.loopAddr} ${db.toFixed(1)}`)
              : undefined
          }
        />
        <LoopToggle
          on={p.looped}
          onToggle={
            p.loopAddr
              ? (on) =>
                  void p.command(
                    `event loop ${p.loopAddr} ${on ? "on" : "off"}`,
                  )
              : undefined
          }
        />
        {/* Trim OPENS a gate rather than applying one, drawing what would be KEPT over the wave (docs/shell.md). */}
        <TrimGate
          open={p.gateOpen}
          onOpenChange={p.onGateOpen}
          busy={p.busy === "trim"}
          disabled={!!p.busy || !p.clip}
          gate={p.gate}
          onGate={p.onGate}
          keep={p.keep}
          canTrim={p.canTrim}
          onTrim={p.onTrim}
        />
        {/* Operations: every chain the project and the library offer, run over THIS clip (@studio/commands docs/processChain.md). */}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="sm" variant="outline" disabled={!!p.busy || !p.clip}>
              {p.busy === "process" ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                m["clip.operations"]()
              )}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64 p-1">
            <ChainOpsList
              search={p.opSearch}
              onSearch={p.onOpSearch}
              ops={p.ops}
              pending={p.opsPending}
              onPick={p.onPickOp}
            />
          </DropdownMenuContent>
        </DropdownMenu>
        {p.onDiscard && (
          <Button
            size="sm"
            variant="outline"
            onClick={p.onDiscard}
            title={m["clip.discardHint"]()}
            aria-label={m["clip.discard"]()}
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        )}
        {p.onDone && (
          <Button size="sm" onClick={p.onDone} title={m["clip.doneHint"]()}>
            <Check className="h-3.5 w-3.5" />
            {m["clip.done"]()}
          </Button>
        )}
        {p.editingOnsets && (
          <OnsetThreshold value={p.sensitivity} onChange={p.onSensitivity} />
        )}
      </div>
    </DockHeaderRow>
  );
}
