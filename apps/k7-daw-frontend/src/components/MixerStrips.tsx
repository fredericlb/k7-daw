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
import { TrackToggle } from "@studio/ui/track-toggle";
import type { TrackControlValues } from "@studio/ui-chains/graph";
import { useRef } from "react";
import { m } from "../paraglide/messages.js";
import type { WebProject } from "../types";

type Toggle = (id: string, control: "mute" | "solo") => void;
type Set = (id: string, control: "volume" | "pan", value: number) => void;

const ZONE = { ok: "bg-meter-ok", hot: "bg-meter-hot", clip: "bg-meter-clip" };

/** The strip's live level and the loudest of the pass, a ref written during render like the master meter's. */
function Meter({ index, muted }: { index: number; muted: boolean }) {
  const db = useEngineState((s) => Math.round(s.levels[index] ?? FLOOR_DB));
  const shown = muted ? FLOOR_DB : db;
  return (
    <div
      className="min-h-24 w-1.5 flex-1 overflow-hidden rounded-[3px] bg-(--k7-progress-track)"
      role="meter"
      aria-label={m["mixer.level"]()}
      aria-valuenow={shown}
    >
      <div
        className={cn("h-full origin-bottom", ZONE[meterZone(shown)])}
        style={{ transform: `scaleY(${meterFill(shown)})` }}
      />
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
        format={(v) => `${Math.round((v * 2 - 1) * 100)}`}
        onValueChange={(v) => onSet?.(id, "pan", v)}
      />
      <Meter index={index} muted={muted} />
      <Knob
        label={m["mixer.volume"]({ name })}
        size={40}
        value={values.volume}
        min={-60}
        max={6}
        step={0.5}
        format={(v) => `${v.toFixed(1)} dB`}
        peak={peak.current > FLOOR_DB ? peak.current.toFixed(1) : "–"}
        clipping={peak.current >= 0}
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
        {m["mixer.masterSum"]({ n: lanes })}
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
        aria-label={m["mixer.level"]()}
        aria-valuenow={shown}
      >
        <div
          className={cn("h-full origin-bottom", ZONE[meterZone(shown)])}
          style={{ transform: `scaleY(${meterFill(shown)})` }}
        />
      </div>
      <Knob
        label={m["shell.masterVolume"]()}
        size={40}
        value={volume}
        min={-60}
        max={6}
        step={0.5}
        format={(v) => `${v.toFixed(1)} dB`}
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
          lanes={strips.length}
          volume={master.volume}
          onVolume={master.onVolume}
          mastering={master.mastering}
        />
      )}
    </ul>
  );
}
