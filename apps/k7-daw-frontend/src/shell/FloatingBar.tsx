// FloatingBar.tsx — the bar floating over the panes: the command bar button, the tool, the transport capsule
// (buttons, clock, song overview, tempo, metronome, master volume) and, at its right end, the current pane's views. On a phone the clock,
// overview, tempo and volume move to a strip above the capsule, and the tool and metronome to the overflow menu. It holds no state of its own: App hands it all.
import { CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  Check,
  ChevronUp,
  Columns3,
  Eraser,
  Image,
  type LucideIcon,
  Maximize,
  MousePointer2,
  Network,
  Rows3,
  Scissors,
  Spline,
  SquareTerminal,
  VolumeX,
  ZoomIn,
  ZoomOut,
} from "@studio/ui/icons";
import type { ComponentProps, ReactNode, Ref } from "react";
import { MetronomeButton } from "../components/MetronomeButton";
import { Minimap } from "../components/Minimap";
import type { RecInfo } from "../hooks/useProjectDoc";
import { ModeIcon } from "../modeIcons";
import { m } from "../paraglide/messages.js";
import { TransportClock } from "../shellMeta";
import { TOOLS, type Tool } from "../timelineTools";
import type { ShellLayout } from "../useIsMobile";
import { summonChord } from "./modKey";
import { OutputRecordButton } from "./OutputRecordButton";
import { ShellTransport } from "./ShellTransport";
import { TempoMeter } from "./TempoMeter";

export const TOOL_NAME: Record<Tool, () => string> = {
  pointer: () => m["shell.tool.pointer"](),
  cut: () => m["shell.tool.cut"](),
  erase: () => m["shell.tool.erase"](),
  mute: () => m["shell.tool.mute"](),
  crossfade: () => m["shell.tool.crossfade"](),
};

const TOOL_ICON: Record<Tool, LucideIcon | null> = {
  pointer: MousePointer2,
  cut: Scissors,
  erase: Eraser,
  mute: VolumeX,
  crossfade: null,
};

export function ToolGlyph({ tool }: { tool: Tool }) {
  const Icon = TOOL_ICON[tool];
  return Icon ? <Icon /> : <ModeIcon mode="crossfade" className="size-5" />;
}

export interface BarLaneViews {
  expanded: boolean;
  automation: boolean;
  /** Only on a project with a picture lane: the picture card's toggle. */
  picture: boolean | null;
  onLanes: () => void;
  onAutomation: () => void;
  onPicture: () => void;
  /** The timeline's zoom, the keys z, Shift+Z and 0 as buttons. */
  onZoomIn: () => void;
  onZoomOut: () => void;
  onZoomFit: () => void;
}

export interface BarGraphViews {
  onFit: () => void;
  /** Absent where the host offers no auto layout. */
  onOrganize?: () => void;
  onStack?: () => void;
  stacked: boolean;
}

export function FloatingBar({
  layout,
  onCommandBar,
  tool,
  onTool,
  transport,
  output,
  recInfo,
  clockSeconds,
  onClock,
  map,
  meter,
  metronome,
  lanes,
  graph,
  hidden,
  surfaceOnly = false,
  runCommand,
  rootRef,
}: {
  /** The bar's root, so the editor popin can measure how tall it is. */
  rootRef?: Ref<HTMLDivElement>;
  layout: ShellLayout;
  onCommandBar: () => void;
  /** The current tool, or null on a project with no arrangement to press on. */
  tool: Tool | null;
  onTool: (t: Tool) => void;
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
  /** The arrangement's capsule; null on any other pane. */
  lanes: BarLaneViews | null;
  /** The Graph pane's capsule; null on any other pane (docs/shell.md, "The capsule follows the pane"). */
  graph: BarGraphViews | null;
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
  // The phone bar carries no tool: it lives in the overflow menu.
  const toolButton =
    tool && !phone ? (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            aria-label={m["shell.tool"]({ name: TOOL_NAME[tool]() })}
            title={m["shell.tool"]({ name: TOOL_NAME[tool]() })}
            className={`float pointer-events-auto flex h-14 shrink-0 cursor-pointer items-center justify-center gap-1 rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring) [&_svg]:size-5 ${phone ? "w-14" : "w-[68px]"}`}
          >
            <ToolGlyph tool={tool} />
            {!phone && <ChevronUp />}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start" className="w-[260px]">
          {TOOLS.map((t) => (
            <DropdownMenuItem key={t} onSelect={() => onTool(t)}>
              <span className="grid size-5 place-items-center text-primary-text">
                <ToolGlyph tool={t} />
              </span>
              {TOOL_NAME[t]()}
              {t === tool && <Check className="ml-auto size-5 text-lavender" />}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    ) : null;

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

  const laneViews = lanes ? (
    <div
      role="group"
      aria-label={m["shell.laneViews"]()}
      className="float pointer-events-auto flex h-14 shrink-0 items-center gap-0.5 rounded-full p-1.5"
    >
      <CapsuleButton
        pressed={lanes.expanded}
        aria-label={m["shell.lane.lanes"]()}
        title={m["shell.lane.lanes"]()}
        onClick={lanes.onLanes}
      >
        <Rows3 />
      </CapsuleButton>
      <CapsuleButton
        pressed={lanes.automation}
        aria-label={m["shell.lane.automation"]()}
        title={m["shell.lane.automation"]()}
        onClick={lanes.onAutomation}
      >
        <Spline />
      </CapsuleButton>
      <CapsuleDivider />
      <CapsuleButton
        aria-label={m["shell.lane.zoomOut"]()}
        title={m["shell.lane.zoomOut"]()}
        onClick={lanes.onZoomOut}
      >
        <ZoomOut />
      </CapsuleButton>
      <CapsuleButton
        aria-label={m["shell.lane.zoomIn"]()}
        title={m["shell.lane.zoomIn"]()}
        onClick={lanes.onZoomIn}
      >
        <ZoomIn />
      </CapsuleButton>
      <CapsuleButton
        aria-label={m["shell.lane.zoomFit"]()}
        title={m["shell.lane.zoomFit"]()}
        onClick={lanes.onZoomFit}
      >
        <Maximize />
      </CapsuleButton>
      {lanes.picture !== null && (
        <CapsuleButton
          pressed={lanes.picture}
          aria-label={m["shell.lane.pictureName"]()}
          title={m["shell.lane.pictureName"]()}
          onClick={lanes.onPicture}
        >
          <Image />
        </CapsuleButton>
      )}
    </div>
  ) : null;

  const graphViews = graph ? (
    <div
      role="group"
      aria-label={m["shell.graphViews"]()}
      className="float pointer-events-auto flex h-14 shrink-0 items-center gap-0.5 rounded-full p-1.5"
    >
      <CapsuleButton
        aria-label={m["graph.zoomFitName"]()}
        title={m["graph.zoomFitName"]()}
        onClick={graph.onFit}
      >
        <Maximize />
        {m["graph.zoomFit"]()}
      </CapsuleButton>
      {graph.onOrganize && (
        <CapsuleButton
          aria-label={m["graph.autoLayoutName"]()}
          title={m["graph.autoLayoutName"]()}
          onClick={graph.onOrganize}
        >
          <Network />
          {m["graph.autoLayout"]()}
        </CapsuleButton>
      )}
      {graph.onStack && (
        <CapsuleButton
          pressed={graph.stacked}
          aria-label={m["graph.stackName"]()}
          title={m["graph.stackName"]()}
          onClick={graph.onStack}
        >
          <Columns3 />
          {m["graph.stack"]()}
        </CapsuleButton>
      )}
    </div>
  ) : null;

  return (
    <div
      ref={rootRef}
      className={`pointer-events-none absolute inset-x-4 z-(--k7-z-bar) flex items-end gap-3 ${phone ? "bottom-7" : "bottom-4"}`}
    >
      {!phone && commandButton}
      {toolButton}
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
      {!phone && (laneViews ?? graphViews)}
      {phone && commandButton}
    </div>
  );
}
