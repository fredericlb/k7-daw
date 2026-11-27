// MixerStrips.tsx — the Mixer dock's strips, left to right: one 84 px card per mixer module, fed by mixerStrips.ts.
// M and S are the lane header's own state (the same `trackControls`); a shared bus strip is ONE card for its first track.

import {
  FLOOR_DB,
  meterFill,
  meterZone,
  peakHold,
} from "@studio/engine-browser/meter";
import { useEngineState } from "@studio/engine-browser/store";
import { Knob } from "@studio/ui/knob";
import { cn } from "@studio/ui/lib/utils";
import { Slider } from "@studio/ui/slider";
import { TrackToggle } from "@studio/ui/track-toggle";
import type { TrackControlValues } from "@studio/ui-chains/graph";
import { useRef } from "react";
import { m } from "../paraglide/messages.js";
import type { WebProject } from "../types";

type Toggle = (id: string, control: "mute" | "solo") => void;
type Set = (id: string, control: "volume" | "pan", value: number) => void;

/** The top of a meter, a little above full scale like the fader's own range. */
const METER_MAX_DB = 6;

/** A pan value (0 left to 1 right) as L / C / R and how far, the way a console prints it. */
export function panText(v: number): string {
  const n = Math.round((v * 2 - 1) * 100);
  if (n === 0) return m["mixer.panCenter"]();
  return n < 0 ? m["mixer.panLeft"]({ n: -n }) : m["mixer.panRight"]({ n });
}

const ZONE = { ok: "bg-meter-ok", hot: "bg-meter-hot", clip: "bg-meter-clip" };

/** The strip's live level and the loudest of the pass, a ref written during render like the master meter's. */
function Meter({
  index,
  muted,
  name,
}: {
  index: number;
  muted: boolean;
  name: string;
}) {
  const db = useEngineState((s) => Math.round(s.levels[index] ?? FLOOR_DB));
  const shown = muted ? FLOOR_DB : db;
  return (
    <div
      className="min-h-24 w-1.5 flex-1 overflow-hidden rounded-[3px] bg-(--k7-progress-track)"
      role="meter"
      aria-label={m["mixer.levelOf"]({ name })}
      aria-valuemin={FLOOR_DB}
      aria-valuemax={METER_MAX_DB}
      aria-valuenow={shown}
    >
      <div
        className={cn("h-full origin-bottom", ZONE[meterZone(shown)])}
        style={{ transform: `scaleY(${meterFill(shown)})` }}
      />
    </div>
  );
}

/** The strip's level: a horizontal fader, −60 to +6 dB, with its value under it; the arrow keys step it by half a dB. */
function Fader({
  label,
  value,
  onValueChange,
}: {
  label: string;
  value: number;
  onValueChange: (db: number) => void;
}) {
  return (
    <div className="flex w-full flex-col items-center gap-1">
      <Slider
        min={-60}
        max={6}
        step={0.5}
        value={value}
        label={label}
        format={(v) => `${v.toFixed(1)} dB`}
        onValueChange={onValueChange}
        className="w-full"
      />
      <span className="type-mono-value text-fg-2">{value.toFixed(1)} dB</span>
    </div>
  );
}

function Strip({
  id,
  name,
  index,
  values,
  onToggle,
  onSet,
}: {
  id: string;
  name: string;
  index: number;
  values: TrackControlValues;
  onToggle?: Toggle;
  onSet?: Set;
}) {
  const db = useEngineState((s) => s.levels[index] ?? FLOOR_DB);
  const playing = useEngineState((s) => s.playing);
  const was = useRef(false);
  const peak = useRef(FLOOR_DB);
  peak.current = peakHold(peak.current, db, playing, was.current);
  was.current = playing;
  const muted = values.mute === 1;
  return (
    <li className="flex w-[84px] shrink-0 flex-col items-center self-stretch gap-2 rounded-2xl bg-(--k7-row) px-1.5 py-2.5">
      <span className="type-label w-full truncate text-center" title={name}>
        {name}
      </span>
      <Knob
        label={m["mixer.pan"]({ name })}
        size={36}
        value={values.pan}
        min={0}
        max={1}
        step={0.02}
        format={panText}
        onValueChange={(v) => onSet?.(id, "pan", v)}
      />
      <span
        className={cn(
          "type-badge rounded-full px-1.5 py-0.5 tabular-nums",
          peak.current >= 0 ? "bg-error text-ink" : "bg-on-fill text-fg-2",
        )}
      >
        {peak.current > FLOOR_DB
          ? (peak.current >= 0 ? m["mixer.peakClip"] : m["mixer.peakHeld"])({
              db: peak.current.toFixed(1),
            })
          : "–"}
      </span>
      <Meter index={index} muted={muted} name={name} />
      <Fader
        label={m["mixer.volume"]({ name })}
        value={values.volume}
        onValueChange={(v) => onSet?.(id, "volume", v)}
      />
      <div className="flex gap-1">
        <TrackToggle
          kind="mute"
          on={muted}
          aria-label={m["mixer.mute"]({ name })}
          onClick={() => onToggle?.(id, "mute")}
        />
        <TrackToggle
          kind="solo"
          on={values.solo === 1}
          aria-label={m["mixer.solo"]({ name })}
          onClick={() => onToggle?.(id, "solo")}
        />
      </div>
    </li>
  );
}

/** Master: the last strip of the row, the sum of every lane; its OUT opens the mastering controls below the row. */
function MasterStrip({
  lanes,
  volume,
  onVolume,
  mastering,
}: {
  lanes: number;
  volume: number;
  onVolume?: (db: number) => void;
  mastering?: { open: boolean; onToggle: () => void };
}) {
  const db = useEngineState((s) => s.master);
  const playing = useEngineState((s) => s.playing);
  const was = useRef(false);
  const peak = useRef(FLOOR_DB);
  peak.current = peakHold(peak.current, db, playing, was.current);
  was.current = playing;
  const shown = Math.round(db);
  return (
    <li
      aria-label={m["graph.master"]()}
      className="flex w-[84px] shrink-0 flex-col items-center self-stretch gap-2 rounded-2xl bg-(--k7-row) px-1.5 py-2.5"
    >
      <span className="type-label w-full truncate text-center">
        {m["graph.master"]()}
      </span>
      <span className="type-secondary w-full truncate text-center text-fg-2">
        {(lanes === 1 ? m["mixer.masterSum.one"] : m["mixer.masterSum.other"])({
          n: lanes,
        })}
      </span>
      <span
        className={cn(
          "type-secondary rounded-full bg-on-fill px-2 tabular-nums",
          peak.current >= 0 && "text-meter-clip",
        )}
      >
        {peak.current > FLOOR_DB ? peak.current.toFixed(1) : "–"}
      </span>
      <div
        className="min-h-24 w-1.5 flex-1 overflow-hidden rounded-[3px] bg-(--k7-progress-track)"
        role="meter"
        aria-label={m["mixer.levelOf"]({ name: m["graph.master"]() })}
        aria-valuemin={FLOOR_DB}
        aria-valuemax={METER_MAX_DB}
        aria-valuenow={shown}
      >
        <div
          className={cn("h-full origin-bottom", ZONE[meterZone(shown)])}
          style={{ transform: `scaleY(${meterFill(shown)})` }}
        />
      </div>
      <Fader
        label={m["shell.masterVolume"]()}
        value={volume}
        onValueChange={(v) => onVolume?.(v)}
      />
      {mastering ? (
        <button
          type="button"
          aria-expanded={mastering.open}
          title={m["mixer.masterOpen"]()}
          onClick={mastering.onToggle}
          className="type-label rounded-full px-2 py-0.5 text-foreground hover:bg-on-fill aria-expanded:bg-on-fill aria-expanded:text-lavender"
        >
          {m["mixer.masterOut"]()}
        </button>
      ) : (
        <span className="type-label text-fg-2">{m["mixer.masterOut"]()}</span>
      )}
    </li>
  );
}

/** The strips of `strips` (mixer module names), each for the first track that owns it. */
export function MixerStrips({
  strips,
  project,
  trackControls,
  onTrackToggle,
  onTrackSet,
  master,
}: {
  strips: readonly string[];
  /** Draws Master as the row's last strip: its fader and, when the song masters, the OUT toggle. */
  master?: {
    volume: number;
    onVolume?: (db: number) => void;
    mastering?: { open: boolean; onToggle: () => void };
  };
  project: WebProject | null | undefined;
  trackControls?: Record<string, TrackControlValues>;
  onTrackToggle?: (id: string, control: "mute" | "solo") => void;
  onTrackSet?: (id: string, control: "volume" | "pan", value: number) => void;
}) {
  const tracks = project?.tracks ?? [];
  // Master sums lanes, not cards: a shared bus strip stands for every track feeding it.
  const laneCount = tracks.filter(
    (t) => t.mixer && strips.includes(t.mixer.module),
  ).length;
  return (
    <ul
      aria-label={m["mixer.strips"]()}
      className="m-0 flex h-full min-h-0 shrink-0 list-none gap-2 p-0 pb-1"
    >
      {strips.map((mod) => {
        const index = tracks.findIndex((t) => t.mixer?.module === mod);
        const track = tracks[index];
        const values = track ? trackControls?.[track.id] : undefined;
        if (!track || !values) return null;
        return (
          <Strip
            key={mod}
            id={track.id}
            name={track.name || track.id}
            index={index}
            values={values}
            onToggle={onTrackToggle}
            onSet={onTrackSet}
          />
        );
      })}
      {master && (
        <MasterStrip
          lanes={laneCount}
          volume={master.volume}
          onVolume={master.onVolume}
          mastering={master.mastering}
        />
      )}
    </ul>
  );
}
