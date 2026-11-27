// MetronomeTempo.tsx — the bar's metronome with the bpm under its icon; a click opens a bubble above it holding the
// tempo (with Tap), the time signature and the metronome's switch and level. Written through project tempo / meter.
import { METRONOME_MIN } from "@studio/engine-protocol/build";
import { Button } from "@studio/ui/button";
import { CapsuleButton } from "@studio/ui/capsule";
import { Input } from "@studio/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@studio/ui/popover";
import { Slider } from "@studio/ui/slider";
import { Switch } from "@studio/ui/switch";
import { useEffect, useRef, useState } from "react";
import type { RecInfo } from "../hooks/useProjectDoc";
import { m } from "../paraglide/messages.js";
import { MetronomeIcon } from "../shellMeta";
import { tapTempo } from "./tapTempo";

const UNITS = [1, 2, 4, 8, 16, 32] as const;
const tempoOk = (v: number) => Number.isFinite(v) && v >= 20 && v <= 400;

export function MetronomeTempo({
  on,
  level,
  onToggle,
  onLevel,
  recInfo,
  runCommand,
}: {
  on: boolean;
  /** 0.2 … 1, from the compiled doc. */
  level: number;
  onToggle: () => void;
  /** Commit a new level: one command, so one journal line. */
  onLevel: (level: number) => void;
  recInfo: RecInfo;
  runCommand: (line: string) => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [tempo, setTempo] = useState(String(recInfo.tempo));
  const [beats, setBeats] = useState(String(recInfo.beatsPerBar));
  const [dragging, setDragging] = useState<number | null>(null);
  const taps = useRef<number[]>([]);
  // Each opening starts from the document, not from a draft left by the last one.
  useEffect(() => {
    if (!open) return;
    setTempo(String(recInfo.tempo));
    setBeats(String(recInfo.beatsPerBar));
    setDragging(null);
    taps.current = [];
  }, [open, recInfo.tempo, recInfo.beatsPerBar]);

  const commitTempo = (v = tempo) => {
    const n = Number(v);
    if (tempoOk(n) && n !== recInfo.tempo)
      void runCommand(`project tempo ${n}`);
  };
  const commitMeter = (b = beats, unit = recInfo.beatUnit) => {
    const n = Math.round(Number(b));
    if (n < 1 || n > 32) return;
    if (n !== recInfo.beatsPerBar || unit !== recInfo.beatUnit)
      void runCommand(`project meter ${n}/${unit}`);
  };
  const commitLevel = () => {
    if (dragging === null) return;
    const v = dragging;
    setDragging(null);
    if (Math.abs(v - level) > 1e-6) onLevel(v);
  };
  const shownLevel = dragging ?? level;
  const label = m["metro.bubble.open"]({ bpm: recInfo.tempo });

  return (
    <Popover
      open={open}
      onOpenChange={(o) => {
        // Closing keeps a typed tempo that was never confirmed with Enter.
        if (!o) commitTempo();
        setOpen(o);
      }}
    >
      <PopoverTrigger asChild>
        <CapsuleButton
          pressed={on}
          aria-label={label}
          title={label}
          className="h-11 flex-col gap-0 px-2 leading-none desk:h-11"
        >
          <MetronomeIcon />
          <span className="type-mono-value text-[11px] tabular-nums">
            {recInfo.tempo}
          </span>
        </CapsuleButton>
      </PopoverTrigger>
      <PopoverContent side="top" className="flex w-64 flex-col gap-3">
        <div className="flex items-center gap-2">
          <span className="type-label w-20">{m["metro.bubble.tempo"]()}</span>
          <Input
            type="number"
            min={20}
            max={400}
            value={tempo}
            aria-label={m["transport.tempoAsk"]()}
            onChange={(e) => setTempo(e.target.value)}
            onBlur={() => commitTempo()}
            onKeyDown={(e) => e.key === "Enter" && commitTempo()}
            className="h-8 w-20 tabular-nums"
          />
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              const r = tapTempo(taps.current, performance.now());
              taps.current = r.taps;
              if (r.bpm !== null) setTempo(String(r.bpm));
            }}
          >
            {m["metro.bubble.tap"]()}
          </Button>
        </div>
        <div className="flex items-center gap-2">
          <span className="type-label w-20">{m["metro.bubble.meter"]()}</span>
          <Input
            type="number"
            min={1}
            max={32}
            value={beats}
            aria-label={m["metro.bubble.beats"]()}
            onChange={(e) => setBeats(e.target.value)}
            onBlur={() => commitMeter()}
            onKeyDown={(e) => e.key === "Enter" && commitMeter()}
            className="h-8 w-14 tabular-nums"
          />
          <span className="text-muted-foreground">/</span>
          <select
            value={recInfo.beatUnit}
            aria-label={m["metro.bubble.unit"]()}
            onChange={(e) => commitMeter(beats, Number(e.target.value))}
            className="h-8 rounded-md border border-border bg-transparent px-2 tabular-nums"
          >
            {UNITS.map((u) => (
              <option key={u} value={u}>
                {u}
              </option>
            ))}
          </select>
        </div>
        <label className="flex items-center gap-2">
          <span className="type-label w-20">{m["transport.metronome"]()}</span>
          <Switch size="sm" checked={on} onCheckedChange={() => onToggle()} />
        </label>
        <label className="flex items-center gap-2 text-xs">
          <span className="type-label w-20">{m["metro.volume"]()}</span>
          <Slider
            min={METRONOME_MIN}
            max={1}
            step={0.05}
            value={shownLevel}
            onValueChange={setDragging}
            onValueCommit={commitLevel}
            className="flex-1"
            label={m["metro.volumeLabel"]()}
          />
          <span className="w-8 text-right tabular-nums">
            {Math.round(shownLevel * 100)}%
          </span>
        </label>
      </PopoverContent>
    </Popover>
  );
}
