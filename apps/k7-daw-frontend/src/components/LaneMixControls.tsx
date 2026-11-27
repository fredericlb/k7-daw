// LaneMixControls.tsx — a lane's own card in the chain dock's title bar, in place of the lane node: "vol" and
// "pan" as properties whose bubble holds a knob, like the waveform's gain, then M, S and A as the mixer's toggles.
// A knob moves locally and COMMITS once, on release, the way a drag on the lane card did (docs/chainsGraph.md).

import { DockKnobProperty } from "@studio/ui/dock-property";
import { TrackToggle } from "@studio/ui/track-toggle";
import type { TrackControlValues } from "@studio/ui-chains/graph";
import { m } from "../paraglide/messages.js";

// The lane card's own ranges: the engine's fader clamps at +6 dB, and pan runs 0 (left) to 1 (right).
const VOL = { min: -60, max: 6, step: 0.1, defaultValue: 0 };
const PAN = { min: 0, max: 1, step: 0.01, defaultValue: 0.5 };

const volLabel = (db: number) => `${db > 0 ? "+" : ""}${db.toFixed(1)} dB`;
const panLabel = (p: number) => {
  const side = Math.round((p - 0.5) * 200);
  return side === 0 ? "C" : side < 0 ? `L${-side}` : `R${side}`;
};

/** The lane's mixer controls, drawn at the right of the title bar. */
export function LaneMixControls({
  name,
  values,
  onToggle,
  onSet,
}: {
  name: string;
  values: TrackControlValues;
  onToggle: (control: "mute" | "solo" | "arm") => void;
  onSet?: (control: "volume" | "pan", value: number) => void;
}) {
  return (
    <>
      {onSet && (
        <>
          <DockKnobProperty
            name={m["chain.vol"]()}
            value={values.volume}
            {...VOL}
            format={volLabel}
            onCommit={(v) => onSet("volume", v)}
          />
          <DockKnobProperty
            name={m["chain.pan"]()}
            value={values.pan}
            {...PAN}
            format={panLabel}
            onCommit={(v) => onSet("pan", v)}
          />
        </>
      )}
      <TrackToggle
        kind="mute"
        on={values.mute === 1}
        aria-label={m["chain.lane.mute"]({ name })}
        title={m["chain.lane.mute"]({ name })}
        onClick={() => onToggle("mute")}
      />
      <TrackToggle
        kind="solo"
        on={values.solo === 1}
        aria-label={m["chain.lane.solo"]({ name })}
        title={m["chain.lane.solo"]({ name })}
        onClick={() => onToggle("solo")}
      />
      <TrackToggle
        kind="arm"
        on={values.arm === 1}
        aria-label={m["chain.armInput"]({ name })}
        title={m["chain.armInput"]({ name })}
        onClick={() => onToggle("arm")}
      />
    </>
  );
}
