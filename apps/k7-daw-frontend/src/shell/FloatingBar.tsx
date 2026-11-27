// FloatingBar.tsx — the transport capsule (buttons, clock, song overview, metronome, master volume) on one floating row, drawn small.
// A desk's sits at the bottom left, its views and command bar button apart; a phone's spans the width, ends with the rail's chevron
// and drops the overview in portrait. The tool is the timeline's corner (ToolMenu.tsx). It holds no state of its own: App hands it all.
import { CapsuleDivider } from "@studio/ui/capsule";
import { SquareTerminal } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { ComponentProps, ReactNode, Ref } from "react";
import { Minimap } from "../components/Minimap";
import type { RecInfo } from "../hooks/useProjectDoc";
import { m } from "../paraglide/messages.js";
import { TransportClock } from "../shellMeta";
import type { ShellLayout } from "../useIsMobile";
import { MetronomeTempo } from "./MetronomeTempo";
import { summonChord } from "./modKey";
import { OutputRecordButton } from "./OutputRecordButton";
import { ShellTransport } from "./ShellTransport";
import { TempoMeter } from "./TempoMeter";

// 56 px controls drawn at 40 px: the bar is a small floating row on a desk or a pad, as the view capsule is.
const BAR_ZOOM = 40 / 56;

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
  railChevron,
  noOverview = false,
}: {
  /** A phone in portrait has no width for the song overview. */
  noOverview?: boolean;
  /** A phone's rail chevron, at its bar's right end where the command bar's button stood (the button moves into the rail). */
  railChevron?: ReactNode;
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
  metronome: Pick<
    ComponentProps<typeof MetronomeTempo>,
    "on" | "level" | "onToggle" | "onLevel"
  > | null;
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
      className="float pointer-events-auto grid size-14 shrink-0 cursor-pointer place-items-center rounded-full text-fg-1 outline-none focus-visible:shadow-(--k7-focus-ring) [&_svg]:size-6"
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
      <Minimap {...map} />
    </div>
  ) : null;
  const tempo = <TempoMeter recInfo={recInfo} runCommand={runCommand} />;

  const buttons = transport ? (
    <ShellTransport {...transport} />
  ) : output ? (
    <OutputRecordButton {...output} />
  ) : null;

  const capsule = (
    <div
      className={cn(
        "float pointer-events-auto flex h-14 min-w-0 items-center gap-1 rounded-full p-1.5",
        phone && !noOverview && "flex-1",
      )}
    >
      {buttons}
      {clock && <CapsuleDivider />}
      {clock}
      {!noOverview && overview}
      {(transport || metronome) && <CapsuleDivider />}
      {metronome ? (
        <MetronomeTempo
          {...metronome}
          recInfo={recInfo}
          runCommand={runCommand}
        />
      ) : (
        tempo
      )}
      {meter && <CapsuleDivider />}
      {meter}
    </div>
  );

  // One row, drawn small (zoomed to 40 px): a desk lifts it above the dock with --k7-float-bottom, a phone spans its width.
  const row = (
    <>
      <div
        role="group"
        aria-label={m["transport.region"]()}
        className={cn(
          "pointer-events-none flex min-w-0 items-stretch",
          phone && !noOverview && "flex-1",
        )}
      >
        {capsule}
      </div>
      {/* The capsule stretches only for the overview to fill; without it the chevron is pushed to the edge instead. */}
      {phone && <span className="flex-1" />}
      {phone && (railChevron ?? commandButton)}
    </>
  );
  if (phone)
    return (
      <div
        ref={rootRef}
        className="pointer-events-none absolute inset-x-3 bottom-3 z-(--k7-z-bar)"
      >
        <div className="flex items-end gap-2" style={{ zoom: BAR_ZOOM }}>
          {row}
        </div>
      </div>
    );
  return (
    <div
      ref={rootRef}
      className="pointer-events-none absolute bottom-(--k7-float-bottom) left-4 z-(--k7-z-bar) transition-[bottom] duration-(--k7-dur-panel)"
    >
      <div className="flex items-end gap-3" style={{ zoom: BAR_ZOOM }}>
        {row}
      </div>
    </div>
  );
}
