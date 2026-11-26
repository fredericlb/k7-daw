// FloatingBar.tsx — the bar of the transport capsule, a phone's command bar button
// (buttons, clock, song overview, tempo, metronome, master volume); the pane's views float apart (ViewCapsule.tsx). On a phone the clock,
// overview, tempo and volume move to a strip above the capsule; a desk or a pad docks it in the header's row, and the metronome to the overflow menu; the tool is the timeline's corner (ToolMenu.tsx), a phone's menu. It holds no state of its own: App hands it all.
import { CapsuleDivider } from "@studio/ui/capsule";
import { SquareTerminal } from "@studio/ui/icons";
import type { ComponentProps, ReactNode, Ref } from "react";
import { MetronomeButton } from "../components/MetronomeButton";
import { Minimap } from "../components/Minimap";
import type { RecInfo } from "../hooks/useProjectDoc";
import { m } from "../paraglide/messages.js";
import { TransportClock } from "../shellMeta";
import type { ShellLayout } from "../useIsMobile";
import { summonChord } from "./modKey";
import { OutputRecordButton } from "./OutputRecordButton";
import { ShellTransport } from "./ShellTransport";
import { TempoMeter } from "./TempoMeter";

export function FloatingBar({
  layout,
  onCommandBar,
  transport,
  output,
  recInfo,
  clockSeconds,
  onClock,
  map,
  meter,
  metronome,
  hidden,
  surfaceOnly = false,
  runCommand,
  rootRef,
  docked = false,
}: {
  /** Drawn in the header's row instead of floating over the panes' foot (a desk or a pad). */
  docked?: boolean;
  /** The bar's root, so the editor popin can measure how tall it is. */
  rootRef?: Ref<HTMLDivElement>;
  layout: ShellLayout;
  onCommandBar: () => void;
  /** The transport's buttons, or null where they would drive nothing on screen (the desk alone). */
  transport: ComponentProps<typeof ShellTransport> | null;
  /** The output recorder's props, for a project with no arrangement. */
  output: ComponentProps<typeof OutputRecordButton> | null;
  recInfo: RecInfo;
  /** The clock reads seconds, and so do the rulers; pressing it switches both. */
  clockSeconds: boolean;
  onClock: () => void;
  map: ComponentProps<typeof Minimap> | null;
  meter: ReactNode;
  metronome: ComponentProps<typeof MetronomeButton> | null;
  /** The bar steps aside (an editor popin over everything). */
  hidden?: boolean;
  /** A surface of another device: the transport capsule and its clock alone, no tool, no views, no command button. */
  surfaceOnly?: boolean;
  runCommand: (line: string) => Promise<void>;
}) {
  if (hidden) return null;
  const phone = layout === "phone";
  if (surfaceOnly)
    return (
      <div
        ref={rootRef}
        className={`pointer-events-none absolute inset-x-4 z-(--k7-z-bar) flex justify-center ${phone ? "bottom-7" : "bottom-4"}`}
      >
        <div
          role="group"
          aria-label={m["transport.region"]()}
          className="float pointer-events-auto flex h-14 min-w-0 items-center gap-1 rounded-full p-1.5"
        >
          {transport && <ShellTransport {...transport} />}
          <CapsuleDivider />
          <TransportClock
            tempo={recInfo.tempo}
            beatsPerBar={recInfo.beatsPerBar}
            seconds={clockSeconds}
            onToggle={onClock}
          />
        </div>
      </div>
    );
  const label = m["shell.commandBar"]();
  const commandButton = (
    <button
      type="button"
      onClick={onCommandBar}
      aria-label={label}
      title={m["shell.commandBar.tip"]({
        name: m["commandBar.list"](),
        keys: summonChord(),
      })}
      className="float pointer-events-auto grid size-14 shrink-0 cursor-pointer place-items-center rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring) [&_svg]:size-6"
    >
      <SquareTerminal />
    </button>
  );

  const clock = map ? (
    <TransportClock
      tempo={recInfo.tempo}
      beatsPerBar={recInfo.beatsPerBar}
      seconds={clockSeconds}
      onToggle={onClock}
    />
  ) : null;
  const overview = map ? (
    <div
      role="img"
      aria-label={m["shell.overview"]()}
      className={phone ? "min-w-0 flex-1" : "flex w-[260px] shrink-0"}
    >
      <Minimap {...map} unfoldDown={docked} />
    </div>
  ) : null;
  const tempo = <TempoMeter recInfo={recInfo} runCommand={runCommand} />;

  const buttons = transport ? (
    <ShellTransport {...transport} />
  ) : output ? (
    <OutputRecordButton {...output} />
  ) : null;

  const capsule = (
    <div className="float pointer-events-auto flex h-14 min-w-0 items-center gap-1 rounded-full p-1.5">
      {buttons}
      {!phone && (
        <>
          {clock && <CapsuleDivider />}
          {clock}
          {overview}
          {(transport || metronome) && <CapsuleDivider />}
          {tempo}
          {metronome && <MetronomeButton {...metronome} />}
          {meter && <CapsuleDivider />}
          {meter}
        </>
      )}
    </div>
  );

  return (
    <div
      ref={rootRef}
      className={
        docked
          ? "pointer-events-none flex min-w-0 flex-1 items-center gap-3"
          : `pointer-events-none absolute inset-x-4 z-(--k7-z-bar) flex items-end gap-3 ${phone ? "bottom-7" : "bottom-4"}`
      }
    >
      <div
        role="group"
        aria-label={m["transport.region"]()}
        className="pointer-events-none flex min-w-0 flex-col items-stretch gap-2"
      >
        {phone && transport && (
          <div className="float pointer-events-auto flex h-[38px] items-center gap-2 rounded-full px-3.5">
            {clock}
            {overview}
            {tempo}
            {meter}
          </div>
        )}
        {capsule}
      </div>
      <span className="flex-1" />
      {phone && commandButton}
    </div>
  );
}
