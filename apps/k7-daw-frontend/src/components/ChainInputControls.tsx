// ChainInputControls.tsx — a lane's input in the chain dock's title bar, in place of the input node: the
// Input menu (the audio sockets and the MIDI devices), then "monitor" and "width" beside it, like the
// waveform editor's gain and loop. Each pick is one `node param` on the input's module (docs/chainsGraph.md).

import { Capsule, CapsuleButton } from "@studio/ui/capsule";
import { DockProperty } from "@studio/ui/dock-property";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import type { LaneInput } from "../laneInput";
import { m } from "../paraglide/messages.js";

/** What a pick asks for: the input kind it needs, and the field and value it then writes. */
export interface InputPick {
  kind: "audio" | "midi";
  field: "channel" | "device";
  value: string;
}

const MONITORS = ["off", "auto", "on"] as const;

// The trigger reads as a title-bar property ("input: In 1"), the shape DockProperty gives gain and loop.
const TRIGGER =
  "type-secondary inline-flex h-8 shrink-0 cursor-pointer items-center gap-1 whitespace-nowrap rounded-2xl px-2.5 outline-none hover:bg-(--k7-row) focus-visible:shadow-(--k7-focus-ring) data-[state=open]:bg-(--k7-row)";

/** The input menu, drawn at the left of the title bar. */
export function ChainInputMenu({
  input,
  audioSets,
  midiDevices,
  onPick,
  onReason,
}: {
  input: LaneInput | null;
  audioSets: readonly { name: string; channels: number[] }[];
  midiDevices: readonly string[];
  onPick: (pick: InputPick) => void;
  onReason?: (message: string) => void;
}) {
  const stereo = input?.channel?.length === 2;
  // An audio input offers the sockets of its own width: the other width is the Width property's question.
  const sets =
    input?.kind === "audio"
      ? audioSets.filter((d) => d.channels.length === (stereo ? 2 : 1))
      : audioSets;
  const current =
    input?.kind === "audio"
      ? (audioSets.find(
          (d) => d.channels.join(",") === input.channel?.join(","),
        )?.name ?? input.channel?.join("+"))
      : input?.kind === "midi"
        ? input.device || m["chain.inputAnyDevice"]()
        : input?.kind === "camera"
          ? input.module
          : m["chain.inputNone"]();
  // A pick of the other kind would need this input removed first, so it says so instead of doing half of it.
  const pick = (p: InputPick) =>
    input && input.kind !== p.kind
      ? onReason?.(m["chain.inputTaken"]())
      : onPick(p);
  const other = (kind: "audio" | "midi") =>
    input && input.kind !== kind ? true : undefined;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger className={TRIGGER}>
        <span className="text-fg-2">{m["chain.input"]()}:</span>
        <span className="type-mono-value text-foreground">{current}</span>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start">
        <DropdownMenuLabel>{m["chain.inputAudio"]()}</DropdownMenuLabel>
        {sets.map((d) => (
          <DropdownMenuItem
            key={d.channels.join(",")}
            aria-disabled={other("audio")}
            onSelect={() =>
              pick({
                kind: "audio",
                field: "channel",
                value: d.channels.join(","),
              })
            }
          >
            {d.name}
          </DropdownMenuItem>
        ))}
        <DropdownMenuSeparator />
        <DropdownMenuLabel>{m["chain.inputMidi"]()}</DropdownMenuLabel>
        {["any", ...midiDevices].map((d) => (
          <DropdownMenuItem
            key={d}
            aria-disabled={other("midi")}
            onSelect={() => pick({ kind: "midi", field: "device", value: d })}
          >
            {d === "any" ? m["chain.inputAnyDevice"]() : d}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** An audio input's monitoring and width, as the title bar's properties. */
export function ChainInputProps({
  input,
  onField,
}: {
  input: LaneInput | null;
  onField: (field: "monitor" | "channel", value: string) => void;
}) {
  if (input?.kind !== "audio") return null;
  const monitor = input.monitor ?? "off";
  const first = input.channel?.[0] ?? 1;
  // A pair is even/odd — the interface groups no other way, as the input card's Width does.
  const odd = first % 2 === 1 ? first : first - 1;
  const stereo = input.channel?.length === 2;
  const monitorWord = (w: (typeof MONITORS)[number]) =>
    w === "auto"
      ? m["chain.monitorAuto"]()
      : w === "on"
        ? m["dock.on"]()
        : m["dock.off"]();
  return (
    <>
      <DockProperty name={m["chain.monitor"]()} value={monitorWord(monitor)}>
        <Capsule aria-label={m["chain.monitor"]()}>
          {MONITORS.map((w) => (
            <CapsuleButton
              key={w}
              pressed={monitor === w}
              onClick={() => monitor !== w && onField("monitor", w)}
            >
              {monitorWord(w)}
            </CapsuleButton>
          ))}
        </Capsule>
      </DockProperty>
      <DockProperty
        name={m["chain.width"]()}
        value={stereo ? m["chain.stereo"]() : m["chain.mono"]()}
      >
        <Capsule aria-label={m["chain.width"]()}>
          <CapsuleButton
            pressed={!stereo}
            onClick={() => stereo && onField("channel", String(first))}
          >
            {m["chain.mono"]()}
          </CapsuleButton>
          <CapsuleButton
            pressed={stereo}
            onClick={() => !stereo && onField("channel", `${odd},${odd + 1}`)}
          >
            {m["chain.stereo"]()}
          </CapsuleButton>
        </Capsule>
      </DockProperty>
    </>
  );
}
