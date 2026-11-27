// ClipControls.tsx — the audio clip editor's options: the event's gain, Loop, the Operations menu (and Discard and
// Done for a take just filed). They are the TITLE BAR's, like every editor's (docs/editors.md); a phone keeps them in the body.

import { Button } from "@studio/ui/button";
import { DockProperty } from "@studio/ui/dock-property";
import { DockHeaderRow } from "@studio/ui/editor-dock";
import { Check, Trash2 } from "@studio/ui/icons";
import { Switch } from "@studio/ui/switch";
import { m } from "../paraglide/messages.js";
import { OnsetThreshold } from "./ClipGateControls";
import { ClipOpsMenu } from "./ClipOpsMenu";
import { EventGainSlider } from "./EventGainSlider";

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
      <div className="ml-auto flex shrink-0 flex-wrap items-center gap-2 text-xs text-fg-2">
        <EventGainSlider
          db={p.gainDb}
          onCommit={
            p.loopAddr
              ? (db) =>
                  void p.command(`event gain ${p.loopAddr} ${db.toFixed(1)}`)
              : undefined
          }
        />
        {p.loopAddr && (
          <DockProperty
            name={m["clip.loop"]().toLowerCase()}
            value={p.looped ? m["dock.on"]() : m["dock.off"]()}
          >
            <Switch
              checked={p.looped}
              aria-label={m["clip.loop"]()}
              title={p.looped ? m["clip.loopOff"]() : m["clip.loopOn"]()}
              onCheckedChange={(on) =>
                void p.command(`event loop ${p.loopAddr} ${on ? "on" : "off"}`)
              }
            />
          </DockProperty>
        )}
        {/* Trim and the chain transformations, behind one lightning button as on the roll (docs/clipEditors.md). */}
        <ClipOpsMenu
          busy={p.busy}
          disabled={!!p.busy || !p.clip}
          gateOpen={p.gateOpen}
          onGateOpen={p.onGateOpen}
          gate={p.gate}
          onGate={p.onGate}
          keep={p.keep}
          canTrim={p.canTrim}
          onTrim={p.onTrim}
          opSearch={p.opSearch}
          onOpSearch={p.onOpSearch}
          ops={p.ops}
          opsPending={p.opsPending}
          onPickOp={p.onPickOp}
        />
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
