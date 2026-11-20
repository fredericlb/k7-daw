// ClipMarksList.tsx — the List tool: every mark of the event as a row, so a mark too close to its neighbour to hit is still reachable.
// Window edges are read-only here; an onset removes with its x, and a stretch point's row opens the same menu its pin does.

import { X } from "@studio/ui/icons";
import { RowGroup } from "@studio/ui/row-group";
import { m } from "../paraglide/messages.js";

export interface MarkRow {
  key: string;
  kind: "edge" | "onset" | "stretch";
  sec: number;
  /** Where a stretch point lands, in seconds into the file; absent for the others. */
  lands?: number;
  /** The stretch point's address for its menu: its source second. */
  src?: number;
}

const KIND = {
  edge: () => m["clip.mark.edge"](),
  onset: () => m["clip.mark.onset"](),
  stretch: () => m["clip.mark.stretch"](),
} as const;

const TONE = {
  edge: "bg-lavender/20 text-lavender",
  onset: "bg-teal-400/20 text-foreground",
  stretch: "bg-sky-400/20 text-foreground",
} as const;

/** `onRemove` is given a row and the point to anchor a menu at; it is undefined for a read-only row. */
export function ClipMarksList({
  rows,
  onFocus,
  onRemove,
}: {
  rows: readonly MarkRow[];
  onFocus: (sec: number) => void;
  onRemove?: (row: MarkRow, x: number, y: number) => void;
}) {
  if (!rows.length)
    return (
      <p className="py-6 text-center text-fg-2 text-sm">
        {m["clip.mark.none"]()}
      </p>
    );
  return (
    <RowGroup className="min-h-0 flex-1 overflow-y-auto">
      {rows.map((r) => (
        <div
          key={r.key}
          className="flex min-h-11 items-center gap-3 px-3.5 py-1.5"
        >
          <button
            type="button"
            onClick={() => onFocus(r.sec)}
            className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 text-left focus-visible:outline-none"
          >
            <span
              className={`rounded-full px-2.5 py-0.5 font-bold text-[11px] ${TONE[r.kind]}`}
            >
              {KIND[r.kind]()}
            </span>
            <span className="font-mono text-[13px]">{r.sec.toFixed(3)} s</span>
            {r.lands != null && (
              <span className="font-mono text-[12px] text-fg-3">
                → {r.lands.toFixed(3)} s
              </span>
            )}
          </button>
          {onRemove && r.kind !== "edge" && (
            <button
              type="button"
              aria-label={m["clip.mark.remove"]()}
              title={m["clip.mark.remove"]()}
              onClick={(e) => {
                const b = e.currentTarget.getBoundingClientRect();
                onRemove(r, b.left, b.bottom);
              }}
              className="grid size-11 shrink-0 cursor-pointer place-items-center rounded-full text-fg-2 hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none"
            >
              <X className="size-4" />
            </button>
          )}
        </div>
      ))}
    </RowGroup>
  );
}
