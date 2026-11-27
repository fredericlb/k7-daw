// EventGainSlider.tsx — an audio event's own trim, ±6 dB: "gain: 0.0 dB" in the clip panel's title bar, a knob in its bubble.
// The knob moves locally and the value COMMITS once, on release — one `event gain`, one write, one reload —
// and a double-click puts it back to unity. See apps/k7-daw-wasm/song-edit/docs/eventGain.md.

import { DockKnobProperty } from "@studio/ui/dock-property";
import { m } from "../paraglide/messages.js";

const MAX_DB = 6;

const label = (db: number) => `${db > 0 ? "+" : ""}${db.toFixed(1)} dB`;

export function EventGainSlider({
  db,
  onCommit,
}: {
  /** The event's trim as the file states it, 0 when it states none. */
  db: number;
  /** Absent ⇒ nothing is drawn: only a placed audio event has a gain of its own. */
  onCommit?: (db: number) => void;
}) {
  if (!onCommit) return null;
  return (
    <DockKnobProperty
      name={m["clip.gain"]().toLowerCase()}
      label={m["clip.gain"]()}
      title={m["clip.gainHint"]()}
      value={db}
      min={-MAX_DB}
      max={MAX_DB}
      step={0.1}
      defaultValue={0}
      format={label}
      onCommit={onCommit}
    />
  );
}
