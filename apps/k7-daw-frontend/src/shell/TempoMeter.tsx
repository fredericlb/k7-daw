// TempoMeter.tsx — the project's tempo and meter, each number editing itself on a click.
// The floating bar draws it, inside the Transport group.

import { tapTempo } from "@studio/k7-wasm/reads";
import { askText } from "@studio/project-shell/ask";
import { Button } from "@studio/ui/button";
import type { RecInfo } from "../hooks/useProjectDoc";
import { m } from "../paraglide/messages.js";

/** The bpm question, with a Tap button (and Space) that fills the field from the taps' tempo. */
function askTempo(initial: number): Promise<string | null> {
  let taps: number[] = [];
  return askText({
    title: m["transport.tempoAsk"](),
    initial: String(initial),
    validate: (v) =>
      Number(v) >= 20 && Number(v) <= 400 ? "" : m["transport.tempoBad"](),
    tool: {
      label: m["transport.tempoTap"](),
      key: " ",
      press: (v) => {
        const r = tapTempo(taps, performance.now());
        taps = r.taps;
        return r.bpm === null ? v : String(r.bpm);
      },
    },
  });
}

/** The doc's tempo and meter, not the engine's: what the metronome clicks, written through `project tempo` / `project meter`. */
export function TempoMeter({
  recInfo,
  runCommand,
}: {
  recInfo: RecInfo;
  runCommand: (line: string) => Promise<void>;
}) {
  return (
    <span className="type-mono-value flex items-center gap-0.5 whitespace-nowrap tabular-nums">
      <Button
        variant="ghost"
        size="bar"
        className="type-mono-value tabular-nums"
        title={m["transport.tempoEdit"]()}
        onClick={() =>
          void askTempo(recInfo.tempo).then((v) => {
            if (v) void runCommand(`project tempo ${v.trim()}`);
          })
        }
      >
        {recInfo.tempo} {m["transport.bpm"]()}
      </Button>

      <Button
        variant="ghost"
        size="bar"
        className="type-mono-value tabular-nums"
        title={m["transport.meterEdit"]()}
        onClick={() =>
          void askText({
            title: m["transport.meterAsk"](),
            initial: `${recInfo.beatsPerBar}/${recInfo.beatUnit}`,
            validate: (v) =>
              /^\d+\/(1|2|4|8|16|32)$/.test(v.trim())
                ? ""
                : m["transport.meterBad"](),
          }).then((v) => {
            if (v) void runCommand(`project meter ${v.trim()}`);
          })
        }
      >
        {recInfo.beatsPerBar}/{recInfo.beatUnit}
      </Button>
    </span>
  );
}
