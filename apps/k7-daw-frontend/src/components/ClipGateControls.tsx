// ClipGateControls.tsx — the clip panel's two thresholds: the onset editor's sensitivity, and the
// trim gate, whose title-bar pill OPENS a gate popover rather than applying one so the wave can draw what it would keep.
// See ../../docs/clipEditors.md.

import { Button } from "@studio/ui/button";
import { Loader2 } from "@studio/ui/icons";
import { Popover, PopoverContent, PopoverTrigger } from "@studio/ui/popover";
import { m } from "../paraglide/messages.js";

// The bounds the file service clamps to anyway (see the /waveform route), repeated so the slider cannot ask for something silently adjusted.
const SENS_MIN = 0.25;
const SENS_MAX = 8;

// The gate's range, in dBFS: −80 is below anything a take carries and −10 above a signal, so neither end is useless.
const GATE_MIN = -80;
const GATE_MAX = -10;

/** The onset detector's sensitivity; unset reads as the service's default, 2. */
export function OnsetThreshold({
  value,
  onChange,
}: {
  value: number | undefined;
  onChange: (value: number) => void;
}) {
  return (
    <label className="flex items-center gap-2">
      {m["clip.threshold"]()}
      <input
        type="range"
        min={SENS_MIN}
        max={SENS_MAX}
        step={0.25}
        value={value ?? 2}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-32"
        aria-label={m["clip.sensitivityLabel"]()}
      />
      <span className="w-8 tabular-nums">{(value ?? 2).toFixed(2)}</span>
    </label>
  );
}

/** The Trim pill in the title bar and the gate it opens: a threshold, what it would keep, and the trim itself. */
export function TrimGate({
  open,
  onOpenChange,
  busy,
  disabled,
  gate,
  onGate,
  keep,
  canTrim,
  onTrim,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  busy: boolean;
  disabled: boolean;
  gate: number;
  onGate: (db: number) => void;
  /** What a trim at this gate would keep, in file seconds, or null for nothing above it. */
  keep: { from: number; to: number } | null;
  /** Off with no EVENT to name: the verb addresses a placement, and a take being reviewed has none. */
  canTrim: boolean;
  onTrim: () => void;
}) {
  return (
    <Popover open={open} onOpenChange={onOpenChange}>
      <PopoverTrigger asChild>
        <Button
          size="sm"
          variant="outline"
          disabled={disabled}
          aria-pressed={open}
          className={open ? "bg-on-fill text-lavender" : undefined}
        >
          {busy ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            m["clip.trim"]()
          )}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="start"
        className="w-80"
        aria-label={m["clip.trim"]()}
        // The wave under it shows what the gate keeps: a press there must not close the gate.
        onInteractOutside={(e) => e.preventDefault()}
      >
        <div className="flex items-center gap-2 text-xs">
          {m["clip.threshold"]()}
          <input
            type="range"
            min={GATE_MIN}
            max={GATE_MAX}
            step={1}
            value={gate}
            onChange={(e) => onGate(Number(e.target.value))}
            className="flex-1"
            aria-label={m["clip.gateLabel"]()}
          />
          <span className="w-14 tabular-nums">{gate} dB</span>
        </div>
        <div className="mt-2 flex items-center gap-3">
          <p className="min-w-0 flex-1 text-[11px] text-muted-foreground">
            {keep
              ? m["clip.keeps"]({
                  from: keep.from.toFixed(2),
                  to: keep.to.toFixed(2),
                })
              : m["clip.nothingAbove"]()}
            {" · "}
            {m["clip.trimHint"]()}
          </p>
          {/* No warning needed: a trim writes this EVENT's window, not the material (song/docs/materialCut.md). */}
          <Button size="sm" disabled={!canTrim} onClick={onTrim}>
            {m["clip.trim"]()}
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}
