// search-list.tsx — a search field over a list, as a combobox: focus stays in the field, the arrows move a highlight, Enter picks it.
// Ranking is one batched call per query through `setSearchRank` (the app registers Rust's fuzzy scorer); the kit falls back to a substring test.
// The one thing a reader would get wrong: `rank={null}` means the items already match the query, and the list only draws them.

import * as React from "react";
import { EmptyRow } from "./empty-row";
import { Search } from "./icons";
import { cn } from "./lib/utils";

/** One score per string, in order; -1 is no match, a higher score ranks first. */
export type SearchRank = (q: string, strings: readonly string[]) => number[];

const substring: SearchRank = (q, strings) => {
  const needle = q.toLowerCase();
  return strings.map((s) => (s.toLowerCase().includes(needle) ? 0 : -1));
};

let defaultRank: SearchRank = substring;

/** The host's ranker for every list that brings none; returns the unregister. */
export function setSearchRank(fn: SearchRank): () => void {
  defaultRank = fn;
  return () => {
    if (defaultRank === fn) defaultRank = substring;
  };
}

/** The items matching `q`, best first and stable among equals; one `rank` call however many items. */
export function rankItems<T>(
  items: readonly T[],
  q: string,
  text: (item: T) => string,
  rank: SearchRank = defaultRank,
): T[] {
  const needle = q.trim();
  if (!needle) return [...items];
  const scores = rank(needle, items.map(text));
  return items
    .map((item, i) => ({ item, i, s: scores[i] ?? -1 }))
    .filter((r) => r.s >= 0)
    .sort((a, b) => b.s - a.s || a.i - b.i)
    .map((r) => r.item);
}

export function SearchList<T>({
  items,
  text,
  rank,
  query: controlled,
  onQueryChange,
  onPick,
  renderRow,
  heading,
  isSelected,
  itemLabel,
  emptyLabel,
  pending,
  limit = 200,
  columns = 1,
  moreLabel,
  searchLabel,
  placeholder,
  before,
  idle,
  autoFocus,
  className,
  listClassName,
  rowClassName,
}: {
  items: readonly T[];
  /** What a query is matched against; unused when `rank` is null. */
  text?: (item: T) => string;
  /** Another ranker, or null when `items` already match the query. */
  rank?: SearchRank | null;
  /** Given, the host owns the query (and passes `rank={null}` when it filtered `items` with it). */
  query?: string;
  onQueryChange?: (q: string) => void;
  onPick: (item: T) => void;
  renderRow: (item: T, state: { active: boolean }) => React.ReactNode;
  /** A heading drawn before a row, when the row opens a new group. */
  heading?: (item: T, previous: T | undefined) => React.ReactNode;
  isSelected?: (item: T) => boolean;
  /** A row's accessible name, when its drawn text (a label and a path) is not the name it goes by. */
  itemLabel?: (item: T) => string;
  emptyLabel: string;
  /** The items are still being read: the empty row spins instead of saying "none". */
  pending?: boolean;
  /** At most this many rows are drawn; a hundred of fifteen hundred is all anyone scans. */
  limit?: number;
  /** Said under the rows when the list was cut at `limit`. */
  moreLabel?: string;
  /** The rows lie in a grid this many wide: Up and Down step a whole row, and stop at the ends. */
  columns?: number;
  searchLabel: string;
  placeholder?: string;
  /** Something above the field (a caller's own actions). */
  before?: React.ReactNode;
  /** Drawn in place of the list while the query is empty (a tree to browse); the field stays above it. */
  idle?: React.ReactNode;
  autoFocus?: boolean;
  className?: string;
  listClassName?: string;
  rowClassName?: string;
}) {
  const [own, setOwn] = React.useState("");
  const [active, setActive] = React.useState(0);
  const query = controlled ?? own;
  const setQuery = (q: string) => {
    setOwn(q);
    setActive(0);
    onQueryChange?.(q);
  };
  const id = React.useId();
  const list = React.useRef<HTMLDivElement>(null);
  const field = React.useRef<HTMLInputElement>(null);
  // preventScroll: a scroll under a pointer menu is what dismisses it, and a plain focus() can cause one.
  React.useEffect(() => {
    if (autoFocus) field.current?.focus({ preventScroll: true });
  }, [autoFocus]);
  const matched = React.useMemo(
    () =>
      rank === null || !text ? [...items] : rankItems(items, query, text, rank),
    [items, query, text, rank],
  );
  const shown = matched.slice(0, limit);
  // A list the host ranked was capped by its producer, so landing exactly on the limit is a cut too.
  const cut =
    matched.length > limit || (rank === null && matched.length === limit);
  const at = shown.length ? Math.min(active, shown.length - 1) : -1;
  const optionId = (n: number) => `${id}-${n}`;

  React.useEffect(() => {
    if (at >= 0)
      list.current
        ?.querySelector(`[id="${id}-${at}"]`)
        ?.scrollIntoView({ block: "nearest" });
  }, [at, id]);

  const keys = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = shown.length;
      const step = e.key === "ArrowDown" ? columns : -columns;
      if (n && columns > 1) setActive(Math.max(0, Math.min(n - 1, at + step)));
      else if (n) setActive((at + (step > 0 ? 1 : n - 1)) % n);
    } else if (e.key === "Enter" && at >= 0) {
      e.preventDefault();
      onPick(shown[at] as T);
    }
  };

  return (
    <div className={cn("flex min-h-0 flex-col gap-1", className)}>
      {before}
      <div className="relative">
        <Search
          aria-hidden
          className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3"
        />
        <input
          ref={field}
          role="combobox"
          aria-expanded
          aria-controls={id}
          aria-autocomplete="list"
          aria-activedescendant={at >= 0 ? optionId(at) : undefined}
          aria-label={searchLabel}
          placeholder={placeholder ?? searchLabel}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={keys}
          spellCheck={false}
          autoComplete="off"
          className="type-body h-11 w-full rounded-xl border border-(--k7-input-line) bg-(--k7-input-fill) pr-3 pl-9 text-fg-1 outline-none placeholder:text-fg-3 focus:border-(--k7-focus-line) desk:h-8"
        />
      </div>
      {idle !== undefined && !query.trim() ? (
        idle
      ) : (
        <div
          ref={list}
          id={id}
          role="listbox"
          aria-label={searchLabel}
          className={cn("min-h-0 overflow-y-auto", listClassName)}
        >
          {shown.map((item, n) => (
            <React.Fragment key={optionId(n)}>
              {heading?.(item, shown[n - 1])}
              {/* biome-ignore lint/a11y/useKeyWithClickEvents: the field owns the keyboard; the rows are the pointer's. */}
              <div
                role="option"
                tabIndex={-1}
                aria-label={itemLabel?.(item)}
                title={itemLabel?.(item)}
                id={optionId(n)}
                aria-selected={isSelected?.(item) ?? n === at}
                // The press must not take focus from the field, or the arrows would stop reaching it.
                onMouseDown={(e) => e.preventDefault()}
                onMouseMove={() => n !== at && setActive(n)}
                onClick={() => onPick(item)}
                className={cn(
                  "type-body flex min-h-11 w-full cursor-pointer items-center gap-2 rounded-xl px-3 text-left text-fg-1 desk:min-h-8",
                  n === at && "bg-on-fill",
                  isSelected?.(item) && "text-lavender",
                  rowClassName,
                )}
              >
                {renderRow(item, { active: n === at })}
              </div>
            </React.Fragment>
          ))}
          {shown.length === 0 ? (
            <EmptyRow pending={pending}>{emptyLabel}</EmptyRow>
          ) : null}
          {cut && moreLabel ? <EmptyRow>{moreLabel}</EmptyRow> : null}
        </div>
      )}
    </div>
  );
}
