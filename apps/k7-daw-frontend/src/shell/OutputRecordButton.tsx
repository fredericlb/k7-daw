// OutputRecordButton.tsx — the bar's record on a project with no timeline: record what the project sends out,
// and its clock while it does. The recording itself is useOutputRecording's (apps/k7-daw-wasm/song/docs/outputRecording.md).
import { CapsuleButton } from "@studio/ui/capsule";
import { Circle } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";

/** `m:ss` of `ms`. */
const clock = (ms: number): string => {
  const s = Math.floor(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};

export function OutputRecordButton({
  recording,
  filing,
  ready,
  onToggle,
}: {
  recording: boolean;
  /** The take is being filed: the button waits, so a press does not start another over it. */
  filing: boolean;
  /** The engine runs this project's document: before that, a press would reach the last project's recorder. */
  ready: boolean;
  onToggle: () => void;
}) {
  const [since, setSince] = useState<number | null>(null);
  const [now, setNow] = useState(0);
  useEffect(() => {
    if (!recording) return setSince(null);
    const t0 = performance.now();
    setSince(t0);
    setNow(t0);
    const id = setInterval(() => setNow(performance.now()), 500);
    return () => clearInterval(id);
  }, [recording]);
  const label = recording ? m["record.stop"]() : m["record.start"]();
  return (
    <>
      <CapsuleButton
        pressed={recording}
        disabled={filing || !ready}
        onClick={onToggle}
        title={label}
        aria-label={label}
      >
        {filing ? (
          <Spinner />
        ) : (
          <Circle className={recording ? "fill-record text-record" : ""} />
        )}
      </CapsuleButton>
      {since !== null && (
        <span
          role="timer"
          aria-label={m["record.clock"]()}
          data-testid="output-record-clock"
          className="type-mono-value px-1 tabular-nums"
        >
          {clock(now - since)}
        </span>
      )}
    </>
  );
}
