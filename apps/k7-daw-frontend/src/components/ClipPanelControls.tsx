// ClipPanelControls.tsx — the clip editor's LEFT column: the recording card while a take rolls, the
// clip line, gain, Loop, Trim and Ops while one is open, the tool bar and its hint (docs/clipEditors.md).
// The one thing a reader gets wrong: nothing here owns state the wave draws from — the gate is the panel's.

import { projectApi } from "@studio/project-shell/project";
import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Check, Loader2, Square, Trash2 } from "@studio/ui/icons";
import { type ComponentProps, useEffect, useMemo, useState } from "react";
import { clipShownName } from "../clipNames";
import { CLIP_NAME_WORDS } from "../clipNameWords";
import { chainOps, filterOps, processLine } from "../clipOps";
import type { LiveTake } from "../hooks/useLiveEnvelope";
import { m } from "../paraglide/messages.js";
import { WAVE_PRESS, type WaveMode } from "../waveMode";
import { ChainOpsList } from "./ChainOpsList";
import { OnsetThreshold, TrimGateMenu } from "./ClipGateControls";
import { ClipToolBar } from "./ClipToolBar";
import { EventGainSlider } from "./EventGainSlider";
import { LiveSeconds } from "./LiveWaveform";
import { LoopToggle } from "./LoopBrace";

/** The recording status card: count-in or bar:beat, the armed lanes, the elapsed seconds and Stop. */
export function RecordingCard({
  counting,
  beats,
  at,
  armedNames,
  live,
  onStop,
}: {
  counting: boolean;
  beats: number;
  at: { bar: number; beat: number };
  armedNames: string[];
  live: LiveTake;
  onStop?: () => void;
}) {
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-muted-foreground text-xs">
      {counting ? (
        <>
          <span className="text-muted-foreground text-sm uppercase tracking-wider">
            {m["clip.countIn"]()}
          </span>
          <span className="font-mono text-2xl text-red-400 tabular-nums">
            {beats}
          </span>
        </>
      ) : (
        <>
          <span className="text-red-400 text-sm uppercase tracking-wider">
            {m["clip.recording"]()}
          </span>
          <span className="font-mono text-2xl tabular-nums">
            {at.bar}:{at.beat}
          </span>
        </>
      )}
      <span className="truncate">
        {armedNames.length ? armedNames.join(", ") : m["clip.noArmedTrack"]()}
      </span>
      <span className="font-mono tabular-nums">
        <LiveSeconds live={live} />
      </span>
      <Button
        size="sm"
        variant="secondary"
        className="ml-auto"
        onClick={onStop}
        title={m["clip.stopTake"]()}
      >
        <Square className="mr-1 h-3 w-3" /> Stop
      </Button>
    </div>
  );
}

type GateProps = ComponentProps<typeof TrimGateMenu>;

/** What the open clip's controls read and write; the panel keeps the gate and the busy flag, the wave draws from them. */
export interface ClipControlsProps {
  clip: string | null;
  track: string;
  laneName: (id: string) => string;
  view: WaveMode;
  modes: readonly WaveMode[];
  gainDb: number;
  loopAddr: string | null;
  looped: boolean;
  command: (line: string) => Promise<void> | void;
  sensitivity: number | undefined;
  onSensitivity: (v: number) => void;
  gate: Pick<GateProps, "open" | "onOpenChange" | "gate" | "onGate" | "keep">;
  busy: string | null;
  canTrim: boolean;
  onTrim: () => void;
  onProcess: (line: string) => void;
  onDiscard?: () => void;
  onClose: () => void;
}

/** The clip line, its pills, the tool bar and the hint, in the left column's order (docs/clipEditors.md). */
export function ClipControls(p: ClipControlsProps) {
  const [chains, setChains] = useState<string[] | null>(null);
  const [opSearch, setOpSearch] = useState("");
  useEffect(() => {
    let live = true;
    projectApi
      .libraryChains()
      .then((r) => live && setChains(chainOps(r.chains)))
      .catch(() => live && setChains([]));
    return () => {
      live = false;
    };
  }, []);
  // Filtered on the whole ref: a chain is filed by path, and nobody remembers the folder.
  const ops = useMemo(
    () => filterOps(chains ?? [], opSearch),
    [chains, opSearch],
  );
  const { clip, track, view, busy } = p;
  const editingOnsets = view === "onsets";
  return (
    <>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-muted-foreground text-xs">
        <span className="font-medium text-foreground" title={clip ?? ""}>
          {clip &&
            clipShownName(clip, p.laneName, CLIP_NAME_WORDS, p.laneName(track))}
        </span>
        {track ? (
          <span>{m["clip.onTrack"]({ track })}</span>
        ) : (
          <span>{m["clip.offTrack"]()}</span>
        )}
        {editingOnsets && (
          <span className="text-faint">{m["clip.onsetHint"]()}</span>
        )}
        {view === "warp" && (
          <span className="text-faint">{m["clip.warpHint"]()}</span>
        )}
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5 text-muted-foreground text-xs">
        <EventGainSlider
          db={p.gainDb}
          onCommit={
            p.loopAddr
              ? (db) =>
                  void p.command(`event gain ${p.loopAddr} ${db.toFixed(1)}`)
              : undefined
          }
        />
        <LoopToggle
          on={p.looped}
          onToggle={
            p.loopAddr
              ? (on) =>
                  void p.command(
                    `event loop ${p.loopAddr} ${on ? "on" : "off"}`,
                  )
              : undefined
          }
        />
        {/* The threshold belongs to the onset editor alone: under a stretch job a slider rewriting the sidecar would be a trap. */}
        {editingOnsets && (
          <OnsetThreshold value={p.sensitivity} onChange={p.onSensitivity} />
        )}
        <TrimGateMenu
          {...p.gate}
          busy={busy === "trim"}
          disabled={!!busy || !clip}
          canTrim={p.canTrim}
          onTrim={p.onTrim}
        />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button size="sm" variant="outline" disabled={!!busy || !clip}>
              {busy === "process" ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                "operations"
              )}
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="w-64 p-1">
            <ChainOpsList
              search={opSearch}
              onSearch={setOpSearch}
              ops={ops}
              pending={chains === null}
              onPick={(c) => clip && p.onProcess(processLine(clip, c))}
            />
          </DropdownMenuContent>
        </DropdownMenu>
        {p.onDiscard && (
          <Button
            size="sm"
            variant="outline"
            onClick={p.onDiscard}
            title={m["clip.discardHint"]()}
            aria-label={m["clip.discard"]()}
          >
            <Trash2 className="h-3.5 w-3.5" />
          </Button>
        )}
        <Button size="sm" onClick={p.onClose} title={m["clip.done"]()}>
          <Check className="h-3.5 w-3.5" />
        </Button>
      </div>
      {/* The tool axis DISPATCHES `ui wave`, the only way to a tool a finger has. */}
      {clip && (
        <>
          <ClipToolBar
            modes={p.modes}
            current={view}
            onPick={(mode) => void p.command(`ui wave ${mode}`)}
          />
          <p className="text-[12px] text-faint leading-4">{WAVE_PRESS[view]}</p>
        </>
      )}
    </>
  );
}

/** The right column's first row: the ZOOM label, what is shown of the clip, and the two ways out of a narrow window. */
export function ZoomRow({
  shown,
  allBars,
  onAll,
  onEvent,
}: {
  shown: number;
  allBars: number;
  onAll: () => void;
  onEvent?: { run: () => void; from: number; to: number };
}) {
  return (
    <div className="flex shrink-0 items-center gap-2 whitespace-nowrap text-muted-foreground text-xs tabular-nums">
      <span className="font-bold text-[11px] uppercase tracking-wider">
        {m["clip.zoom"]()}
      </span>
      <span>
        {Math.round(shown)}/{allBars} {m["clip.bars"]()}
      </span>
      {shown < allBars && (
        <button
          type="button"
          className="underline hover:text-foreground"
          onClick={onAll}
        >
          {m["clip.allBars"]()}
        </button>
      )}
      {onEvent && (
        <button
          type="button"
          className="underline hover:text-foreground"
          onClick={onEvent.run}
          title={m["clip.backToEvent"]({
            from: onEvent.from.toFixed(2),
            to: onEvent.to.toFixed(2),
          })}
        >
          {m["clip.theEvent"]()}
        </button>
      )}
    </div>
  );
}
