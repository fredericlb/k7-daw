// GraphPanel.tsx — the dock's graph view: the selected track's signal path, answering
// "where does THIS track go?" without leaving the timeline. Same pipeline as the Chains
// tab (subgraph -> buildGraph -> dagre -> DomChains), only the input set differs; dagre
// already lays the path out, so don't hand-roll a strip — it would lose the routed sends.
// The track comes from the caret, so the panel has no selection state of its own. Given
// `strips`, it is the MIXER instead: the strips side by side, Master the last, its controls below the row.

import type { PluginParamsStore } from "@studio/engine-browser/params";
import { ASPECTS, aspectOf } from "@studio/song-schema/aspect";
import { DockHeaderRow, DockHint, DockStatus } from "@studio/ui/editor-dock";
import { Workflow } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { LaneSpan } from "@studio/ui-chains/cables";
import type { TrackControlValues } from "@studio/ui-chains/graph";
import type { LaneNotes } from "@studio/ui-chains/noteLoad";
import { type Cam, DomChains, type TapAction } from "@studio/ui-chains-react";
import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { thawAutoFreeze } from "../api";
import { STUDIO_ICONS } from "../chainsIcons";
import { useChainsWords } from "../chainsLabels";
import { nodePredicate } from "../historyMarks";
import { useLatest } from "../hooks/useLatest";
import { usePendingBypass } from "../hooks/usePendingBypass";
import { useRackTrigger } from "../hooks/useRackTrigger";
import { midiActivity } from "../midiActivity";
import { m } from "../paraglide/messages.js";
import { usePaneGraph } from "../queries/graph";
import { useLiveRackLit } from "../rackLit";
import { useReloadKey, useShell, useVerbs } from "../state/shellStore";
import { layoutTrackPanel } from "../trackPanelLayout";
import type { WebProject } from "../types";
import { useIsMobile } from "../useIsMobile";
import { MixerStrips } from "./MixerStrips";

export function GraphPanel({
  onPreviewNote,
  cameras,
  inputDevices,
  midiDevices,
  organizeKey = 0,
  trackId,
  strips,
  master,
  masterVolume,
  onMaster,
  trackOrder,
  videoSpans,
  noteLanes,
  viewport,
  project,
  onBypassModule,
  onProjectField,
  onAlternative,
  onAssetBrowse,
  onTapEdit,
  onWatchWaveform,
  engineWarnings,
  onProblems,
  pluginParamsStore,
  setPluginParam,
  moduleParams,
  onShaderParam,
  trackControls,
  onTrackToggle,
  onTrackSet,
}: {
  /** Play one note through a lane, as the piano roll's click does — what an SFZ kit's pad sounds through. */
  onPreviewNote?: (track: string, key: number, vel: number) => void;
  /** The lenses this browser can see — a camera card's dropdown, apps/k7-daw-frontend/packages/song/docs/camera.md. */
  cameras?: readonly string[];
  /** The interface's inputs as the engine groups them — what an audio input's "in" dropdown offers. */
  inputDevices?: readonly { name: string; channels: number[] }[];
  /** The MIDI inputs this browser sees, by name — what a midi-in node's dropdown offers. */
  midiDevices?: readonly string[];
  // Bumped when `graph organize` ran — empties the held node positions (see ChainsTab's).
  organizeKey?: number;
  trackId: string | null;
  /** The mixer strips (mixerStrips.ts): set, the panel draws the whole graph lined up on them rather than one track's path. */
  strips?: readonly string[];
  /** The Master section drawn under the strips (the mastering surface): the Mixer panel's last section. */
  master?: ReactNode;
  /** The song's output level in dB and how to write it: Master's fader, the Mixer row's last strip. */
  masterVolume?: number;
  onMaster?: (db: number) => void;
  // The engine's track order; the live-signal set is derived off the store (see ChainsTab).
  trackOrder: string[];
  /** …and when each video lane shows a picture, which nothing meters — straight through, for a subgraph holding one (../videoSpans.ts). */
  videoSpans?: Readonly<Record<string, readonly LaneSpan[]>>;
  /** …and the notes', for a MIDI cable's ripple (../noteSpans.ts). */
  noteLanes?: Readonly<Record<string, LaneNotes>>;
  /** The FRAME's size, so the RENDER card ticks the shape it is (@studio/song-schema/aspect). */
  viewport?: readonly [number, number];
  /** The compiled doc, for the rack pads' lit state — straight through to ChainsTab. */
  project?: WebProject | null;
  // Asks only: the shell's bypass reloads nothing here, so `pendingBypass` is what moves the card.
  onBypassModule?: (module: string) => void;
  /** A RECORDER's button, straight through — see DomChains' prop. */
  /** Set a PROJECT setting — the RENDER card's frame dropdown (../noteSpans.ts's sibling). */
  onProjectField?: (field: string, value: string) => void;
  /** A node's alternative badge → the shell switches to that set, or copies the one on (null). */
  onAlternative?: (module: string, name: string | null) => void;
  /** A file-valued param's browser: the same cards are drawn here, so the same button has to open. */
  onAssetBrowse?: (module: string, kind: string, x: number, y: number) => void;
  /** A rack tap's edit — the shell runs its `node tap` command (apps/k7-daw-frontend/packages/song/docs/racks.md). */
  onTapEdit?: (module: string, action: TapAction) => void;
  /** …and the waveform widget's subscription, for the same reason — it draws nothing unwatched. */
  onWatchWaveform?: (
    pluginId: string,
    side: "input" | "output",
    on: boolean,
  ) => void;
  /** Engine load problems by plugin id, badged onto the module they concern. */
  engineWarnings?: Record<string, string>;
  /** A module's warning badge was clicked: open the problems list. */
  onProblems?: () => void;
  /** The live values, and the way back: without them a widget draws the FILE's number and a drag sends nothing (../../docs/chainsGraph.md). */
  pluginParamsStore: PluginParamsStore;
  setPluginParam: (id: string, name: string, value: number) => void;
  /** …and the third source, for the modules the engine holds NOTHING of — without it a tempo node draws its file's number for ever. */
  moduleParams?: Record<string, Record<string, number>>;
  /** A shader knob turned on a card: `node param` on its node (hooks/useShaderParam.ts). */
  onShaderParam?: (module: string, name: string, value: number) => void;
  trackControls?: Record<string, TrackControlValues>;
  onTrackToggle?: (
    trackId: string,
    control: "mute" | "solo" | "arm" | "highlight",
  ) => void;
  onTrackSet?: (
    trackId: string,
    control: "volume" | "pan",
    value: number,
  ) => void;
}) {
  // What a hovered History row touched, lit here — read from the store so a hover re-renders no one else (historyMarks.ts).
  const mark = useShell((s) => s.historyMark);
  const reloadKey = useReloadKey();
  const labels = useChainsWords();
  // A card writes the quiet way, an intrinsic field the reloading way, and a right-click opens the shell's menu.
  const {
    quietCommand: command,
    runCommand: onModuleField,
    openContextMenu: onContextMenu,
  } = useVerbs();
  // Parked HERE, because App mounts this panel conditionally — one per track, so another track's camera never stands in for a fit.
  const camStores = useRef(new Map<string, { current: Cam | null }>());
  const camKey = trackId ?? "";
  let camStore = camStores.current.get(camKey);
  if (!camStore) {
    camStore = { current: null };
    camStores.current.set(camKey, camStore);
  }
  const [selected, setSelected] = useState<string | null>(null);
  // The shared graph at this reload key, re-asked on a backoff while the service is not answering (queries/graph.ts).
  const { graph, error } = usePaneGraph(reloadKey);

  // The session memory ChainsTab keeps, per TRACK — another track is another picture — and emptied by `graph organize`.
  const prevLaid = useRef<Map<string, { x: number; y: number }> | null>(null);
  const prevScope = useRef<string | number>("");
  /** The ticked row, as a NUMBER and outside the layout memo: the viewport is a fresh array on
   *  every fetch, so depending on it directly would relayout on every poll — and leaving it out
   *  is what left the menu on the old shape (../docs/picture.md). */
  const aspectRow = useMemo(() => {
    const shape = viewport ? aspectOf(viewport[0], viewport[1]) : null;
    return shape ? ASPECTS.findIndex((a) => a.name === shape.name) : undefined;
  }, [viewport]);

  const laid = useMemo(() => {
    if (!graph) return null;
    if (!trackId) return null;
    const scope = `${trackId}:${organizeKey}`;
    if (prevScope.current !== scope) {
      prevScope.current = scope;
      prevLaid.current = null;
    }
    // No pins: they place cards on the whole graph's canvas, and a track's slice of it would be scattered (trackPanelLayout.ts).
    const out = layoutTrackPanel(
      graph,
      trackId,
      {
        cameras,
        inputDevices,
        midiDevices,
        ...(aspectRow === undefined ? {} : { aspect: aspectRow }),
      },
      prevLaid.current ?? undefined,
    );
    if (!out) return null; // nothing routes this track
    prevLaid.current = new Map(
      out.nodes.map((n) => [n.id, { x: n.x, y: n.y }]),
    );
    return out;
  }, [
    graph,
    trackId,
    organizeKey,
    cameras,
    inputDevices,
    midiDevices,
    aspectRow,
  ]);

  // A chain selected on one track means nothing once the caret moves to another.
  useEffect(() => setSelected(null), [trackId]);

  // The History panel's hover, as the per-card test DomChains reads.
  const marked = useMemo(() => nodePredicate(mark), [mark]);
  // The rack pads light here too: this panel draws the same nodes, so it owes them the same answer (rackLit.ts).
  const rackLit = useLiveRackLit(project, graph);
  // …and they PLAY here too, by the same hook as the chains pane.
  const laidRef = useLatest(laid);
  const triggerRack = useRackTrigger(laidRef, onPreviewNote, rackLit.flash);

  // Bypasses clicked but not yet in `graph`, painted on after layout (usePendingBypass.ts).
  const { bypassModule, laidShown } = usePendingBypass(
    laid,
    laidRef,
    graph,
    onBypassModule,
  );

  // The mastering controls open under the strips by default: Master's OUT folds them away.
  const [masteringOpen, setMasteringOpen] = useState(true);

  const body = () => {
    // Only with nothing to draw: a failed RE-read keeps the last graph while useDocument retries.
    if (error && !graph)
      return (
        <DockStatus tone="error">
          {m["graph.unavailable"]({ error })}
        </DockStatus>
      );
    if (!graph) return <DockStatus>{m["common.loading"]()}</DockStatus>;
    if (strips)
      return (
        <>
          <MixerStrips
            strips={strips}
            project={project}
            trackControls={trackControls}
            onTrackToggle={onTrackToggle}
            onTrackSet={onTrackSet}
            master={{
              volume: masterVolume ?? 0,
              onVolume: onMaster,
              mastering: master
                ? {
                    open: masteringOpen,
                    onToggle: () => setMasteringOpen((o) => !o),
                  }
                : undefined,
            }}
          />
          {/* Master is drawn even with no lane to sum, so its fader and OUT stay in reach. */}
          {!strips.length && <DockStatus>{m["graph.noStrip"]()}</DockStatus>}
        </>
      );
    if (!strips && !trackId)
      return <DockStatus>{m["graph.selectTrack"]()}</DockStatus>;
    if (!laidShown)
      return (
        <DockStatus>
          {m["graph.noChainFrom"]({ track: trackId ?? "" })}
        </DockStatus>
      );
    return (
      <DomChains
        labels={labels}
        icons={STUDIO_ICONS}
        graph={laidShown}
        selected={selected}
        trackOrder={trackOrder}
        videoSpans={videoSpans}
        noteLanes={noteLanes}
        rackLit={rackLit}
        midiActivity={midiActivity}
        marked={marked}
        onTriggerRack={triggerRack}
        onSelect={setSelected}
        onContextMenu={onContextMenu}
        // This panel follows the caret's track, so another track IS a different graph and earns a re-frame; a reload isn't.
        fitKey={trackId}
        organizeKey={organizeKey}
        camStore={camStore}
        command={command}
        onBypassModule={bypassModule}
        onThawAutoFreeze={thawAutoFreeze}
        onModuleField={onModuleField}
        onProjectField={onProjectField}
        onAlternative={onAlternative}
        onAssetBrowse={onAssetBrowse}
        onTapEdit={onTapEdit}
        onWatchWaveform={onWatchWaveform}
        engineWarnings={engineWarnings}
        onProblems={onProblems}
        pluginParamsStore={pluginParamsStore}
        setPluginParam={setPluginParam}
        moduleParams={moduleParams}
        onShaderParam={onShaderParam}
        trackControls={trackControls}
        onTrackToggle={onTrackToggle}
        onTrackSet={onTrackSet}
      />
    );
  };

  const mobile = useIsMobile();
  const count = strips?.length ?? 0;
  const title = strips
    ? m["shell.panel.mixer"]()
    : (trackId ?? m["graph.noTrack"]());
  const masterSection =
    strips && master && masteringOpen ? (
      <section
        aria-label={m["graph.master"]()}
        className="max-h-[55%] shrink-0 overflow-auto rounded-2xl bg-(--k7-row) p-3"
      >
        {master}
      </section>
    ) : null;
  // The Mixer is one row, the strips left to right and Master last; its controls open below the row, and the note and the link close it.
  if (strips)
    return (
      <div className="flex h-full min-h-0 flex-col gap-2">
        <DockHeaderRow>
          <p className="flex min-w-0 items-baseline gap-2 truncate">
            <span className="type-label text-foreground">{title}</span>
            <span className="font-mono text-fg-2 text-xs">
              {(count === 1 ? m["graph.strips.one"] : m["graph.strips.other"])({
                n: count,
              })}
            </span>
          </p>
        </DockHeaderRow>
        <DockHeaderRow>
          <button
            type="button"
            onClick={() => void onModuleField("ui graph")}
            aria-label={m["mixer.inGraph"]()}
            title={m["mixer.inGraph"]()}
            className="glass ml-auto grid size-9 shrink-0 cursor-pointer place-items-center rounded-full text-foreground focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
          >
            <Workflow className="size-4" />
          </button>
        </DockHeaderRow>
        <DockHint text={m["mixer.footer"]()} />
        <div className={cn("flex min-h-0 flex-1", mobile && "overflow-x-auto")}>
          <div
            className={cn(
              "flex min-h-0 gap-2",
              mobile ? "shrink-0" : "min-w-0 flex-1 overflow-x-auto",
            )}
          >
            {body()}
          </div>
        </div>
        {masterSection}
      </div>
    );
  return (
    <div className="flex h-full min-h-0 flex-col">
      <DockHeaderRow>
        <p className="truncate font-mono text-fg-2 text-xs">{title}</p>
      </DockHeaderRow>
      <div className="min-h-0 flex-1">{body()}</div>
    </div>
  );
}
