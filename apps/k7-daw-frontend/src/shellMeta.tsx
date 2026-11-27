// shellMeta.tsx — the shell's static tables (panes, views, panels: icon + label) and the small
// presentational pieces built on them: the transport clock, the metronome glyph, and the
// mini transport the floating minimap carries. Apart from App.tsx so the shell reads as
// wiring, not tables.

import { useEngineState } from "@studio/engine-browser/store";
import { formatAt } from "@studio/k7-wasm/gestureTicks";
import {
  AudioWaveform,
  Disc3,
  FileCode,
  type LucideIcon,
  MonitorPlay,
  Music,
  Pause,
  Play,
  ScrollText,
  SlidersVertical,
  Workflow,
} from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { Spinner } from "@studio/ui/spinner";
import { m } from "./paraglide/messages.js";
import type { Pane, Panel } from "./routes";

// The panes, not tabs: a view is a LAYOUT over the same instances, and which of them a project mounts is `viewPanes` in routes.ts, not this table.
export const PANES: Pane[] = ["editor", "timeline", "turntables", "chains"];

// Max gap between the two Shift presses that open the command palette (IntelliJ style).
export const DOUBLE_SHIFT_MS = 400;

// Each `label` is a GETTER, so it is read in the language shown now rather than the one loaded first (docs/settings.md).
export const PANE_META: Record<Pane, { icon: LucideIcon; label: string }> = {
  editor: {
    icon: FileCode,
    get label() {
      return m["pane.editor"]();
    },
  },
  timeline: {
    icon: AudioWaveform,
    get label() {
      return m["pane.timeline"]();
    },
  },
  turntables: {
    icon: Disc3,
    get label() {
      return m["pane.turntables"]();
    },
  },
  chains: {
    icon: Workflow,
    get label() {
      return m["pane.chains"]();
    },
  },
};

// The picture's window — not a pane, so not in the table above, but drawn with the same glyph wherever a surface is listed.
export const VIEWPORT_META: { icon: LucideIcon; label: string } = {
  icon: MonitorPlay,
  get label() {
    return m["shell.lane.picture"]();
  },
};

// No metadata for a layout: there is one, and which panes it holds is what the project declares (routes.ts).

// The bottom panels: `event` is the clip or lane opened, `graph` the caret track's path, `mix` the strips lined up, `logs` what happened.
export const PANEL_META: Record<Panel, { icon: LucideIcon; label: string }> = {
  event: {
    icon: Music,
    get label() {
      return m["panel.event"]();
    },
  },
  graph: {
    icon: Workflow,
    get label() {
      return m["panel.graph"]();
    },
  },
  mix: {
    icon: SlidersVertical,
    get label() {
      return m["panel.mix"]();
    },
  },
  logs: {
    icon: ScrollText,
    get label() {
      return m["panel.logs"]();
    },
  },
};

// The moving clock, its OWN component so the ~20 Hz playhead re-renders one <span>. Needs <EngineStoreProvider>.
export function TransportClock({
  tempo,
  beatsPerBar,
  seconds = false,
  onToggle,
}: {
  tempo: number;
  beatsPerBar: number;
  /** Which timecode it reads: seconds, or bar:beat; the rulers count in the same unit. */
  seconds?: boolean;
  /** Pressing the clock switches the unit, here and on the rulers. */
  onToggle?: () => void;
}) {
  const s = useEngineState((st) => st.playhead);
  // Position as 1-based bar:beat, clamped at 0 so a count-in's negative playhead doesn't read as a negative bar.
  const beat = Math.max(0, (s * tempo) / 60);
  const bpb = Math.max(1, beatsPerBar);
  const bar = Math.floor(beat / bpb) + 1;
  const beatInBar = Math.floor(beat % bpb) + 1;
  const label =
    m[seconds ? "transport.positionSeconds" : "transport.positionBars"]();
  return (
    // A timer that is also the unit's switch: a button would hide the timer role from assistive tech.
    <span
      role="timer"
      aria-label={label}
      title={
        onToggle
          ? m[seconds ? "transport.clockToBars" : "transport.clockToSeconds"]()
          : label
      }
      data-testid="timecode"
      data-unit={seconds ? "seconds" : "bars"}
      tabIndex={onToggle ? 0 : undefined}
      onClick={onToggle}
      onKeyDown={(e) => {
        if (!onToggle || (e.key !== "Enter" && e.key !== " ")) return;
        e.preventDefault();
        onToggle();
      }}
      className={cn(
        "type-mono-timecode mx-1 whitespace-nowrap rounded-md px-1 text-fg-1 tabular-nums",
        onToggle && "cursor-pointer hover:bg-on-fill",
      )}
    >
      {seconds ? formatAt(s) : `${bar}:${beatInBar}`}
    </span>
  );
}

/** The transport reduced to what you need when the shell is hidden: play/pause and the clock, with `stop` as the pause the engine understands. */
export function MiniTransport({
  playing,
  waiting,
  tempo,
  beatsPerBar,
  onToggle,
}: {
  playing: boolean;
  /** The engine is still loading the project: it can answer no transport yet. */
  waiting: boolean;
  tempo: number;
  beatsPerBar: number;
  onToggle: () => void;
}) {
  return (
    <div className="flex shrink-0 items-center gap-1 border-b border-hairline bg-menu px-2 py-1">
      <button
        type="button"
        disabled={waiting}
        onClick={onToggle}
        title={
          waiting
            ? m["status.engineStarting"]()
            : playing
              ? m["transport.pause"]()
              : m["transport.play"]()
        }
        aria-label={
          waiting
            ? m["status.engineStarting"]()
            : playing
              ? m["transport.pause"]()
              : m["transport.play"]()
        }
        aria-busy={waiting}
        className="flex h-6 w-6 items-center justify-center rounded text-fg-1 hover:bg-row disabled:opacity-50"
      >
        {waiting ? (
          <Spinner size="xs" inherit />
        ) : playing ? (
          <Pause className="h-3.5 w-3.5" />
        ) : (
          <Play className="h-3.5 w-3.5" />
        )}
      </button>
      <TransportClock tempo={tempo} beatsPerBar={beatsPerBar} />
    </div>
  );
}

// A metronome glyph hand-drawn in lucide's style (24-grid, currentColor, round caps), since lucide has none.
export function MetronomeIcon({ className }: { className?: string }) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M9 3h6l5 18H4z" />
      <path d="M12 18 16 7" />
      <path d="M8 13h8" />
    </svg>
  );
}
