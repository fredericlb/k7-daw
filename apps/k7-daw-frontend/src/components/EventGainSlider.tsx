// EventGainSlider.tsx — an audio event's own trim, ±6 dB: "gain: 0.0 dB" in the clip panel's title bar, a knob in its bubble.
// The knob moves locally and the value COMMITS once, on release — one `event gain`, one write, one reload —
// and a double-click puts it back to unity. See apps/k7-daw-frontend/packages/song-edit/docs/eventGain.md.

import { DockProperty } from "@studio/ui/dock-property";
import { Knob } from "@studio/ui/knob";
import { useRef, useState } from "react";
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
  const [draft, setDraft] = useState<number | null>(null);
  const last = useRef<number | null>(null);
  if (!onCommit) return null;
  const shown = draft ?? db;
  const commit = (v: number | null) => {
    setDraft(null);
    last.current = null;
    if (v !== null && Math.abs(v - db) > 1e-6) onCommit(v);
  };
  return (
    <DockProperty name={m["clip.gain"]().toLowerCase()} value={label(shown)}>
      {/* The knob says nothing of its release: the gesture's end bubbles here, from a pointer or a key. */}
      {/* biome-ignore lint/a11y/noStaticElementInteractions: the knob inside is the control; this only hears its release */}
      <div
        title={m["clip.gainHint"]()}
        onPointerUp={() => commit(last.current)}
        onKeyUp={() => commit(last.current)}
        onDoubleClick={() => commit(0)}
      >
        <Knob
          value={shown}
          min={-MAX_DB}
          max={MAX_DB}
          step={0.1}
          size={36}
          label={m["clip.gain"]()}
          format={label}
          onValueChange={(v) => {
            last.current = v;
            setDraft(v);
          }}
        />
      </div>
    </DockProperty>
  );
}
