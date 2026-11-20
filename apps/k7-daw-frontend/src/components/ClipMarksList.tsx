// ClipMarksList.tsx — the List tool: every mark of the event as a row, so a mark too close to its neighbour to hit is still reachable.
// A value taps into a field to type another (`onEdit`); an onset removes with its x, and a stretch point's row opens the same menu its pin does.

import { X } from "@studio/ui/icons";
import { RowGroup } from "@studio/ui/row-group";
import { useState } from "react";
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

/** One value in seconds: a button while it reads, a field once tapped; Enter or leaving commits a changed, valid number. */
function SecValue({
  sec,
  arrow,
  onEdit,
}: {
  sec: number;
  arrow?: boolean;
  onEdit?: (sec: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const text = `${arrow ? "→ " : ""}${sec.toFixed(3)} s`;
  const cls = arrow
    ? "font-mono text-[12px] text-fg-3"
    : "font-mono text-[13px]";
  if (!onEdit) return <span className={cls}>{text}</span>;
  if (draft === null)
    return (
      <button
        type="button"
        title={m["clip.mark.edit"]()}
        onClick={() => setDraft(sec.toFixed(3))}
        className={`${cls} cursor-pointer rounded px-1 hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none`}
      >
        {text}
      </button>
    );
  const commit = () => {
    const v = Number(draft.replace(",", "."));
    setDraft(null);
    if (Number.isFinite(v) && v >= 0 && Math.abs(v - sec) > 5e-4) onEdit(v);
  };
  return (
    <input
      // biome-ignore lint/a11y/noAutofocus: the field opens by a tap on the value, so focus follows it
      autoFocus
      inputMode="decimal"
      aria-label={m["clip.mark.edit"]()}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        else if (e.key === "Escape") setDraft(null);
      }}
      className="h-7 w-24 rounded bg-on-fill px-2 font-mono text-[13px] outline-none focus-visible:shadow-(--k7-focus-ring)"
    />
  );
}

/** `onRemove` is given a row and the point to anchor a menu at; it is undefined for a read-only row. */
export function ClipMarksList({
  rows,
  onFocus,
  onRemove,
  onEdit,
}: {
  rows: readonly MarkRow[];
  onFocus: (sec: number) => void;
  /** Commits a typed value for a row — an edge or onset moves, a stretch point's landing moves; undefined for a read-only row. */
  onEdit?: (row: MarkRow, sec: number) => void;
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
          <div className="flex min-w-0 flex-1 items-center gap-3">
            <button
              type="button"
              onClick={() => onFocus(r.sec)}
              className="cursor-pointer text-left focus-visible:outline-none"
            >
              <span
                className={`rounded-full px-2.5 py-0.5 font-bold text-[11px] ${TONE[r.kind]}`}
              >
                {KIND[r.kind]()}
              </span>
            </button>
            {r.lands != null ? (
              <>
                <SecValue sec={r.sec} />
                <SecValue
                  sec={r.lands}
                  arrow
                  onEdit={onEdit && ((v) => onEdit(r, v))}
                />
              </>
            ) : (
              <SecValue sec={r.sec} onEdit={onEdit && ((v) => onEdit(r, v))} />
            )}
          </div>
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
