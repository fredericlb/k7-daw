// BandTable.tsx — the one table the library's songs and the home page's projects are drawn in: a search
// box at its head, columns, rows grouped under bands, and only the rows near the scroll position in the
// page, the rest standing in as two spacer rows. The caller says what a row is; this file draws none.
// What reads wrong: a hidden column is hidden on every row by the column's own class, so a band or a
// spacer has the same cells as a row and never a colSpan, which would add empty columns.

import { Search, X } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@studio/ui/table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, useLayoutEffect, useRef, useState } from "react";

/** A row's height before it is measured; measured rows keep their own. */
const ROW_PX = 44;
/** Rows drawn beyond the visible ones, so a quick scroll shows rows and not a gap. */
const OVERSCAN = 12;

/** The element that scrolls `el`: the nearest ancestor whose content overflows it, else the page. */
function scrollParent(el: HTMLElement | null): HTMLElement | null {
  // An overflow style is not enough: a wrapper with overflow-x, or one that grows with its content, never scrolls down.
  for (let at = el?.parentElement ?? null; at; at = at.parentElement) {
    const y = getComputedStyle(at).overflowY;
    if (
      (y === "auto" || y === "scroll") &&
      at.scrollHeight > at.clientHeight + 1
    )
      return at;
  }
  return document.scrollingElement as HTMLElement | null;
}

/** What a drawn row is handed, to spread on its `<tr>`: the virtualizer's ref and its index. */
export interface Measured {
  ref: (el: HTMLTableRowElement | null) => void;
  "data-index": number;
}

/** One row of the table, drawn only when near the scroll position. */
export interface BandItem {
  key: string;
  draw: (m: Measured) => ReactNode;
}

/** A column: its heading, and the class every cell of it carries (a width, or `hidden sm:table-cell`). */
export interface BandColumn {
  head?: ReactNode;
  className?: string;
}

/** A band's row: the class it carries, and the class of the cell that names it. */
export const BAND_ROW = "hover:bg-transparent";

/** The search box at the table's head: Escape empties it, and so does its ✕. */
export function BandSearch({
  value,
  onChange,
  label,
  clearLabel,
}: {
  value: string;
  onChange: (value: string) => void;
  label: string;
  clearLabel: string;
}) {
  return (
    <div className="relative min-w-48 flex-1">
      <Search className="pointer-events-none absolute top-1/2 left-2.5 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
      {/* Text and not `search`: Chrome draws its own clear button there, a second one beside ours. */}
      <Input
        type="text"
        role="searchbox"
        inputMode="search"
        enterKeyHint="search"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Escape" && value) {
            e.preventDefault();
            onChange("");
          }
        }}
        placeholder={label}
        aria-label={label}
        className="pr-8 pl-8"
      />
      {value ? (
        <button
          type="button"
          className="absolute top-1/2 right-2 -translate-y-1/2 rounded p-0.5 text-muted-foreground hover:text-foreground"
          aria-label={clearLabel}
          title={clearLabel}
          onClick={() => onChange("")}
        >
          <X className="h-4 w-4" />
        </button>
      ) : null}
    </div>
  );
}

/** A band's cells: its label in the first column (`labelClass` styling it), `extra` in the last, the others empty but present. */
export function bandCells(
  columns: BandColumn[],
  label: ReactNode,
  labelClass: string,
  extra: ReactNode = null,
): ReactNode[] {
  return columns.map((c, i) => (
    <TableCell
      // biome-ignore lint/suspicious/noArrayIndexKey: the columns are fixed, their order is their identity.
      key={i}
      className={
        i === 0
          ? labelClass
          : i === columns.length - 1
            ? `pt-4 text-right ${c.className ?? ""}`
            : c.className
      }
    >
      {i === 0 ? label : i === columns.length - 1 ? extra : null}
    </TableCell>
  ));
}

/** The table itself: the columns' heads, then `items` in order, a screenful of them in the page. */
export function BandTable({
  columns,
  items,
}: {
  columns: BandColumn[];
  items: BandItem[];
}) {
  // The body's place in what scrolls, re-read each render: what sits above it may wrap.
  const body = useRef<HTMLTableSectionElement>(null);
  const [scroller, setScroller] = useState<HTMLElement | null>(null);
  const [margin, setMargin] = useState(0);
  useLayoutEffect(() => {
    const el = scrollParent(body.current);
    if (el !== scroller) setScroller(el);
    if (!el || !body.current) return;
    const top =
      body.current.getBoundingClientRect().top -
      (el === document.scrollingElement ? 0 : el.getBoundingClientRect().top) +
      el.scrollTop;
    if (Math.abs(top - margin) > 1) setMargin(top);
  });
  const virtual = useVirtualizer({
    count: items.length,
    getScrollElement: () => scroller,
    estimateSize: () => ROW_PX,
    overscan: OVERSCAN,
    scrollMargin: margin,
    getItemKey: (i) => items[i]?.key ?? i,
  });
  const shown = virtual.getVirtualItems();
  const above = shown.length ? shown[0].start - margin : 0;
  // Before the scroller is known nothing is drawn, and the whole height stands in, so there is an overflow to find.
  const below = shown.length
    ? virtual.getTotalSize() - (shown[shown.length - 1].end - margin)
    : virtual.getTotalSize();
  const spacer = (key: string, height: number) =>
    height > 0 ? (
      <tr key={key} aria-hidden style={{ height }} className="border-0">
        {columns.map((c, i) => (
          // biome-ignore lint/suspicious/noArrayIndexKey: the columns are fixed, their order is their identity.
          <td key={i} className={c.className} />
        ))}
      </tr>
    ) : null;
  return (
    // touch-manipulation: no double-tap zoom, since a double tap opens a row. A 10% hairline: the bands divide the table.
    <Table className="table-fixed touch-manipulation [&_tr]:border-white/10">
      <TableHeader>
        <TableRow>
          {columns.map((c, i) => (
            // biome-ignore lint/suspicious/noArrayIndexKey: the columns are fixed, their order is their identity.
            <TableHead key={i} className={c.className}>
              {c.head}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>
      <TableBody ref={body}>
        {spacer("above", above)}
        {shown.map((v) =>
          items[v.index]?.draw({
            ref: virtual.measureElement,
            "data-index": v.index,
          }),
        )}
        {spacer("below", below)}
      </TableBody>
    </Table>
  );
}
