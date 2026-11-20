// ModuleCards.tsx — the chain dock's module cards, left to right: name, engine chip, bypass, preset with its arrows, a "+" on each seam and a dashed "Add a module" at the end.
// A summary of the lane's row, not a second editor: the controls stay on the full row (the panel's "Controls" chip shows it).

import { ChevronLeft, ChevronRight, Plus, Power } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { EdgeInsert, GraphNode } from "@studio/ui-chains/graph";
import { m } from "../paraglide/messages.js";

export interface CardSeam {
  insert: EdgeInsert;
  end?: boolean;
}

export function ModuleCards({
  nodes,
  seams,
  presetNames,
  onBypass,
  onPresetStep,
  onPresets,
  onInsert,
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
  className?: string;
}) {
  const between = seams.filter((s) => !s.end);
  const tail = seams.find((s) => s.end);
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
      {nodes.map((n, i) => {
        const id = n.module ?? n.id;
        const off = !!n.bypassed || n.dim;
        const preset = presetNames?.[id];
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
                {n.engine && (
                  <span className="rounded-full bg-on-fill px-2.5 py-0.5 font-mono text-[11px] text-lavender">
                    {n.engine}
                  </span>
                )}
                {onBypass && (
                  <button
                    type="button"
                    aria-pressed={!off}
                    aria-label={m["chain.bypass"]({ name: n.label })}
                    title={m["chain.bypass"]({ name: n.label })}
                    onClick={() => onBypass(id)}
                    className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-fg-2 hover:bg-on-fill aria-pressed:text-lavender focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
                  >
                    <Power className="size-5" />
                  </button>
                )}
              </div>
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
                {plus(between[i], m["chain.addBetween"]())}
              </span>
            )}
          </li>
        );
      })}
      <li className="flex shrink-0 items-stretch">
        {plus(tail, m["chain.addModule"](), true)}
      </li>
    </ul>
  );
}
