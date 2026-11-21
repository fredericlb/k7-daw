// ModuleCards.tsx — the chain dock's cards, left to right: the lane's own card, then each module (name, engine chip, warning,
// native UI, bypass, preset with its arrows, three knobs), a "+" on each seam and a dashed "Add a module at the end".
// Not a second editor: the controls stay on the full row (the panel's "Controls" chip), but a card's "⋯" opens the graph's own menu.

import type { PluginParamsStore } from "@studio/engine-browser/params";
import {
  ChevronLeft,
  ChevronRight,
  Ellipsis,
  Plus,
  Power,
  PowerOff,
  SquareArrowOutUpRight,
} from "@studio/ui/icons";
import { Knob } from "@studio/ui/knob";
import { cn } from "@studio/ui/lib/utils";
import { TrackToggle } from "@studio/ui/track-toggle";
import type {
  EdgeInsert,
  GraphNode,
  TrackControlValues,
} from "@studio/ui-chains/graph";
import { m } from "../paraglide/messages.js";
import { ModuleKnobs } from "./ModuleKnobs";
import { LaneWarning } from "./timeline/LaneWarning";

export interface CardSeam {
  insert: EdgeInsert;
  end?: boolean;
}

/** The lane's own card: its status, M/S/A, volume and pan, where the song comes in, and where the row goes. */
export interface LaneCard {
  id: string;
  name: string;
  values?: TrackControlValues;
  onToggle?: (id: string, control: "mute" | "solo" | "arm") => void;
  onSet?: (id: string, control: "volume" | "pan", value: number) => void;
}

/** The module a seam's jack names, whatever port it leaves from or enters by. */
const base = (ref: string) => ref.split("#")[0];

export function ModuleCards({
  nodes,
  seams,
  presetNames,
  onBypass,
  onPresetStep,
  onPresets,
  onInsert,
  lane,
  warningOf,
  onProblems,
  paramsStore,
  onParam,
  onNativeUI,
  onMenu,
  className,
}: {
  /** The row's module cards in signal order. */
  nodes: readonly GraphNode[];
  /** The row's seams in the same order: one after each card, the last (`end`) being the tail. */
  seams: readonly CardSeam[];
  presetNames?: Record<string, string>;
  onBypass?: (module: string) => void;
  onPresetStep?: (module: string, step: -1 | 1) => void;
  onPresets?: (module: string, x: number, y: number) => void;
  onInsert?: (insert: EdgeInsert, x: number, y: number) => void;
  /** The lane's card that opens the row; absent draws the modules alone. */
  lane?: LaneCard;
  /** The engine's warning for a module, drawn as an amber icon on its card. */
  warningOf?: (n: GraphNode) => string | undefined;
  /** The warning icon's click: the Logs panel on its problems. */
  onProblems?: () => void;
  /** The live params and how to write one: the card's three knobs. */
  paramsStore?: PluginParamsStore;
  onParam?: (id: string, name: string, value: number) => void;
  /** A card's native plugin window, for a module that has one. */
  onNativeUI?: (module: string) => void;
  /** A card's "⋯": the menu the graph opens on a right-click (remove, move, open the file). */
  onMenu?: (n: GraphNode, x: number, y: number) => void;
  className?: string;
}) {
  const tail = seams.find((s) => s.end);
  const ids = nodes.map((n) => n.module ?? n.id);
  // Seams are matched by the modules they join, not by position: the lane's own seam would shift every index.
  const after = (i: number) =>
    seams.find(
      (s) =>
        !s.end &&
        base(s.insert.before) === ids[i] &&
        base(s.insert.after) === ids[i + 1],
    );
  const lead = seams.find(
    (s) => !s.end && !s.insert.stack && base(s.insert.after) === ids[0],
  );
  const plus = (s: CardSeam | undefined, label: string, dashed?: boolean) =>
    s && onInsert ? (
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          onInsert(s.insert, r.left, r.bottom);
        }}
        className={cn(
          "flex cursor-pointer items-center justify-center gap-2 text-lavender focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none",
          dashed
            ? "min-h-14 w-52 self-stretch rounded-2xl border-[1.5px] border-hairline border-dashed font-bold text-[14px] hover:bg-on-fill/40"
            : "size-8 rounded-full bg-(--k7-row) hover:bg-on-fill",
        )}
      >
        <Plus className="size-4" />
        {dashed && label}
      </button>
    ) : null;
  return (
    <ul
      className={cn(
        "m-0 flex min-w-0 list-none items-stretch gap-1 overflow-x-auto p-0 pb-1",
        className,
      )}
    >
      {lane && (
        <li className="flex shrink-0 items-stretch gap-1">
          <LaneCardView lane={lane} />
          {lead && (
            <span className="flex items-center">
              {plus(lead, m["chain.addBefore"]())}
            </span>
          )}
        </li>
      )}
      {nodes.map((n, i) => {
        const id = n.module ?? n.id;
        const off = !!n.bypassed || n.dim;
        const preset = presetNames?.[id];
        const warning = warningOf?.(n);
        return (
          <li key={n.id} className="flex shrink-0 items-stretch gap-1">
            <div
              className={cn(
                "flex w-64 flex-col gap-2 rounded-2xl bg-(--k7-row) p-3",
                off && "opacity-60",
              )}
            >
              <div className="flex items-center gap-2">
                <span className="min-w-0 flex-1 truncate font-bold text-[15px]">
                  {n.label}
                </span>
                {warning && (
                  <LaneWarning sentences={[warning]} onOpen={onProblems} />
                )}
                {n.engine && (
                  <span className="rounded-full bg-on-fill px-2.5 py-0.5 font-mono text-[11px] text-lavender">
                    {n.engine}
                  </span>
                )}
                {n.hasNativePluginUI && n.module && onNativeUI && (
                  <button
                    type="button"
                    aria-label={m["chainsUi.openPluginUi"]()}
                    title={m["chainsUi.openPluginUi"]()}
                    onClick={() => n.module && onNativeUI(n.module)}
                    className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-fg-2 hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
                  >
                    <SquareArrowOutUpRight className="size-5" />
                  </button>
                )}
                {n.bypassed && (
                  <span className="rounded-full bg-on-fill px-2.5 py-0.5 text-[11px] text-fg-2">
                    {m["chain.bypassedTag"]()}
                  </span>
                )}
                {onBypass && (
                  <button
                    type="button"
                    aria-pressed={!!n.bypassed}
                    aria-label={m["chain.bypass"]({ name: n.label })}
                    title={m["chain.bypass"]({ name: n.label })}
                    onClick={() => onBypass(id)}
                    className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-fg-2 hover:bg-on-fill aria-pressed:text-lavender focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
                  >
                    {n.bypassed ? (
                      <PowerOff className="size-5" />
                    ) : (
                      <Power className="size-5" />
                    )}
                  </button>
                )}
                {onMenu && (
                  <button
                    type="button"
                    aria-haspopup="menu"
                    aria-label={m["chain.moduleMenu"]({ name: n.label })}
                    title={m["chain.moduleMenu"]({ name: n.label })}
                    onClick={(e) => {
                      const r = e.currentTarget.getBoundingClientRect();
                      onMenu(n, r.left, r.bottom);
                    }}
                    className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-fg-2 hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
                  >
                    <Ellipsis className="size-5" />
                  </button>
                )}
              </div>
              <ModuleKnobs node={n} store={paramsStore} onSet={onParam} />
              {onPresets && (
                <div className="flex items-center gap-1">
                  <button
                    type="button"
                    aria-label={m["chain.presetPrev"]()}
                    onClick={() => onPresetStep?.(id, -1)}
                    className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full hover:bg-on-fill"
                  >
                    <ChevronLeft className="size-5" />
                  </button>
                  <button
                    type="button"
                    aria-label={m["chain.presetOf"]({
                      preset: preset ?? m["chain.preset"](),
                      name: n.label,
                    })}
                    onClick={(e) => {
                      const r = e.currentTarget.getBoundingClientRect();
                      onPresets(id, r.left, r.bottom);
                    }}
                    className="h-11 min-w-0 flex-1 cursor-pointer truncate rounded-full bg-(--k7-row) px-4 text-left text-[14px]"
                  >
                    {preset ?? m["chain.preset"]()}
                  </button>
                  <button
                    type="button"
                    aria-label={m["chain.presetNext"]()}
                    onClick={() => onPresetStep?.(id, 1)}
                    className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full hover:bg-on-fill"
                  >
                    <ChevronRight className="size-5" />
                  </button>
                </div>
              )}
            </div>
            {i < nodes.length - 1 && (
              <span className="flex items-center">
                {plus(after(i), m["chain.addBetween"]())}
              </span>
            )}
          </li>
        );
      })}
      <li className="flex shrink-0 items-stretch">
        {plus(tail, m["chain.addModuleEnd"](), true)}
      </li>
    </ul>
  );
}

function LaneCardView({ lane }: { lane: LaneCard }) {
  const v = lane.values;
  return (
    <div className="flex w-56 flex-col gap-2 rounded-2xl bg-(--k7-row) p-3">
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate font-bold text-[15px]">
          {lane.name}
        </span>
        {v && lane.onToggle && (
          <div className="flex gap-1">
            {/* Arm is the header's: one button for one state. */}
            {(["mute", "solo"] as const).map((c) => (
              <TrackToggle
                key={c}
                kind={c}
                on={v[c] === 1}
                aria-label={m[`chain.lane.${c}`]({ name: lane.name })}
                onClick={() => lane.onToggle?.(lane.id, c)}
              />
            ))}
          </div>
        )}
      </div>
      {v && (
        <div className="flex items-start justify-around gap-1">
          <div className="flex flex-col items-center gap-0.5">
            <Knob
              label={m["mixer.volume"]({ name: lane.name })}
              size={36}
              value={v.volume}
              min={-60}
              max={6}
              step={0.5}
              format={(x) => `${x.toFixed(1)} dB`}
              onValueChange={(x) => lane.onSet?.(lane.id, "volume", x)}
            />
            <span className="type-secondary text-fg-2">VOL</span>
          </div>
          <div className="flex flex-col items-center gap-0.5">
            <Knob
              label={m["mixer.pan"]({ name: lane.name })}
              size={36}
              value={v.pan}
              min={0}
              max={1}
              step={0.02}
              format={(x) => `${Math.round((x * 2 - 1) * 100)}`}
              onValueChange={(x) => lane.onSet?.(lane.id, "pan", x)}
            />
            <span className="type-secondary text-fg-2">PAN</span>
          </div>
        </div>
      )}
      <p className="type-secondary mt-auto flex justify-between text-fg-2">
        <span>{m["chain.songComesIn"]()}</span>
        <span>{m["chain.toMaster"]()}</span>
      </p>
    </div>
  );
}
