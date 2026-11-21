// AudioInterfaceSection.tsx — App settings › Devices: the engine's primary audio interface and the one it falls back on.
// Two selects send the whole `audio.priority` pair; the supervisor opens the first one that is plugged in.

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@studio/ui/select";
import { m } from "../paraglide/messages.js";
import {
  audioChoice,
  type DesktopEngineSnapshot,
  engineCommand,
  idleAsioDriver,
  priorityArg,
} from "./desktopEngine";
import { Action, Row, Section } from "./Section";

/** Stands for "no fallback": Radix Select refuses an empty item value. */
const NONE = "\u0000none";

/** The interface the engine is on, said where the eye lands, with the channels it actually opened. */
function DeviceBadge({
  text,
  inputs,
  outputs,
}: {
  text: string;
  inputs: number;
  outputs: number;
}) {
  const channels = [
    inputs > 0 && m["engine.channelsIn"]({ n: String(inputs) }),
    outputs > 0 && m["engine.channelsOut"]({ n: String(outputs) }),
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <span className="shrink-0 type-badge rounded-full border border-(--k7-ok-line) bg-(--k7-ok-fill) px-2 py-0.5 text-ok">
      {channels ? `${text} · ${channels}` : text}
    </span>
  );
}

function DeviceSelect({
  label,
  value,
  onChange,
  options,
  none,
}: {
  label: string;
  value: string;
  onChange: (name: string) => void;
  options: { name: string; present: boolean }[];
  /** The label of the "no device" item, when the choice may be empty. */
  none?: string;
}) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger aria-label={label} className="w-64 max-w-[45vw]">
        <SelectValue />
      </SelectTrigger>
      <SelectContent position="popper">
        {none && <SelectItem value={NONE}>{none}</SelectItem>}
        {options.map((o) => (
          <SelectItem key={o.name} value={o.name}>
            {o.present ? o.name : `${o.name} · ${m["engine.notConnected"]()}`}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export function AudioInterfaceSection({
  s,
}: {
  s: DesktopEngineSnapshot | null;
}) {
  if (!s) return null;
  const { primary, fallback, options } = audioChoice(s);
  const open = (s.audioDevices ?? []).find((d) => d.current);
  const asio = idleAsioDriver(s);
  const send = (p: string, f: string | null) =>
    engineCommand("audio.priority", priorityArg(p, f));
  return (
    <Section title={m["engine.audioTitle"]()} hint={m["engine.audioHint"]()}>
      {options.length === 0 ? (
        <Row label={m["engine.noDevices"]()} />
      ) : (
        <>
          <Row label={m["engine.primary"]()}>
            <DeviceSelect
              label={m["engine.primary"]()}
              value={primary ?? ""}
              options={options}
              onChange={(name) => send(name, fallback)}
            />
          </Row>
          <Row label={m["engine.fallback"]()}>
            <DeviceSelect
              label={m["engine.fallback"]()}
              value={fallback ?? NONE}
              options={options.filter((o) => o.name !== primary)}
              none={m["engine.fallbackNone"]()}
              onChange={(name) =>
                send(primary ?? name, name === NONE ? null : name)
              }
            />
          </Row>
          {open && (
            <Row
              label={
                <span className="flex items-center gap-2">
                  <span className="truncate">{open.name}</span>
                  <DeviceBadge
                    text={m["engine.inUse"]()}
                    inputs={open.currentInput ? (open.inputs ?? 0) : 0}
                    outputs={open.outputs ?? 0}
                  />
                </span>
              }
              detail={open.deviceType}
            />
          )}
        </>
      )}
      {/* The engine on Windows' shared endpoint while the interface has its own driver: two channels a side, and the mixer's latency. */}
      {asio && (
        <Row
          label={
            <span className="text-warn">
              {m["engine.asioHint"]({ driver: asio })}
            </span>
          }
        >
          <Action onClick={() => send(asio, primary)}>
            {m["engine.asioUse"]()}
          </Action>
        </Row>
      )}
    </Section>
  );
}
