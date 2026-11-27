// TreePicker.tsx — the browsing popup: levels that open sideways on hover or tap, a search field
// at the root (the kit's SearchList), and the same dismissal contract as ContextMenu (the kit's useDismiss).
//
// Generic over what a leaf CARRIES, because several things browse this way and behaving
// differently would be a coin toss for the user: a module (ModulePicker), a file from the
// asset library (AssetPicker), a node's preset (PresetPicker). All are "a nested catalogue,
// searchable"; a caller with actions of its own puts them in `header`, above the search.

import { EmptyRow } from "@studio/ui/empty-row";
import { ChevronRight } from "@studio/ui/icons";
import { useDismiss, usePointClamp } from "@studio/ui/point-menu";
import { SearchList } from "@studio/ui/search-list";
import { useLayoutEffect, useMemo, useRef, useState } from "react";

/** One row: a branch has `children`, a leaf has `entry`, and a row with neither is plain text — which is why both are optional. */
export interface TreeNode<T> {
  key: string;
  label: string;
  children?: TreeNode<T>[];
  entry?: T;
  /** Shown muted on the right of a leaf — the full ref/path, so you see what you pick. */
  hint?: string;
  /** A short tag drawn as a pill BEFORE the label — what kind of thing the leaf is (`NAM`, `AIDA`). */
  badge?: string;
}

// Scrolling and submenus can't share an element — an `overflow` clips a submenu away. They never need to.
const MENU_SCROLL = "max-h-[min(28rem,80vh)] overflow-y-auto overflow-x-hidden";
const ROW =
  "type-label flex min-h-11 w-full cursor-pointer items-center justify-between gap-4 rounded-2xl px-3 text-left outline-none hover:bg-on-fill focus-visible:bg-on-fill desk:min-h-8";

/** What a leaf looks like, in the tree and in the search results alike. */
function Leaf<T>({ n }: { n: TreeNode<T> }) {
  return (
    <>
      <span className="flex min-w-0 items-center gap-2">
        {n.badge ? (
          <span className="w-10 shrink-0 rounded bg-muted px-1 text-center font-mono text-[11px] uppercase text-muted-foreground">
            {n.badge}
          </span>
        ) : null}
        <span
          className={`truncate font-mono ${n.entry || n.children ? "" : "text-muted-foreground"}`}
        >
          {n.label}
        </span>
      </span>
      {n.hint ? (
        <span className="shrink-0 truncate font-mono text-xs text-muted-foreground">
          {n.hint}
        </span>
      ) : null}
    </>
  );
}

/** One level: its rows, and the next level beside whichever is open (hover, focus or a tap). */
function Level<T>({
  nodes,
  onPick,
  label,
  nested,
}: {
  nodes: TreeNode<T>[];
  onPick: (e: T) => void;
  label?: string;
  nested?: boolean;
}) {
  const [open, setOpen] = useState<string | null>(null);
  const [flip, setFlip] = useState(false);
  const subRef = useRef<HTMLDivElement>(null);

  // Flip a submenu left when it would run off the edge, MEASURED: levels differ wildly in width.
  useLayoutEffect(() => {
    const el = subRef.current;
    if (!el) {
      setFlip(false);
      return;
    }
    setFlip(el.getBoundingClientRect().right > window.innerWidth - 8);
  }, [open]);

  // Only the all-leaf levels scroll — see MENU_SCROLL.
  const hasBranches = nodes.some((n) => n.children?.length);

  return (
    <div
      className={
        nested
          ? "menu min-w-[13rem] rounded-(--k7-r-menu) p-1.5 text-sm"
          : hasBranches
            ? ""
            : MENU_SCROLL
      }
      role="menu"
      aria-label={label}
    >
      {nodes.map((n) => {
        const isLeaf = !n.children?.length;
        return (
          <div key={n.key} className="relative">
            <button
              type="button"
              role="menuitem"
              aria-label={n.label}
              aria-description={n.hint}
              aria-haspopup={isLeaf ? undefined : "menu"}
              aria-expanded={isLeaf ? undefined : open === n.key}
              // Hover and focus open a branch; a tap (which hovers first, then clicks) must not close it again.
              onMouseEnter={() => setOpen(isLeaf ? null : n.key)}
              onFocus={() => setOpen(isLeaf ? null : n.key)}
              onClick={() =>
                isLeaf ? n.entry && onPick(n.entry) : setOpen(n.key)
              }
              className={ROW}
            >
              {isLeaf ? (
                <Leaf n={n} />
              ) : (
                <>
                  <span className="truncate font-mono">{n.label}</span>
                  <ChevronRight className="size-3.5 text-muted-foreground" />
                </>
              )}
            </button>
            {open === n.key && n.children?.length ? (
              <div
                ref={subRef}
                className="absolute top-0 z-10"
                style={flip ? { right: "100%" } : { left: "100%" }}
              >
                <Level nodes={n.children} onPick={onPick} nested />
              </div>
            ) : null}
          </div>
        );
      })}
    </div>
  );
}

export function TreePicker<T>({
  x,
  y,
  tree,
  search: runSearch,
  searchLimit,
  placeholder,
  menuLabel,
  searchLabel,
  emptyLabel,
  onPick,
  onClose,
  header,
  aside,
}: {
  x: number; // viewport coords of whatever opened it
  y: number;
  /** The catalogue, already levelled. An empty tree renders nothing at all. */
  tree: TreeNode<T>[];
  /** Flat matches for a query — the caller owns ranking, since only it knows the shape. */
  search: (query: string) => TreeNode<T>[];
  /** The cap `search` applies, so this can say the list was cut rather than look complete. */
  searchLimit: number;
  placeholder: string;
  /** The accessible names of the menu and its search box (the placeholder is not one a restyle may keep). */
  menuLabel?: string;
  searchLabel?: string;
  emptyLabel: string;
  onPick: (pick: T) => void;
  onClose: () => void;
  /** The caller's own row above the search — with one, an empty tree still opens, saying `emptyLabel`. */
  header?: React.ReactNode;
  /** A column LEFT of the list, inside the popup — a choice the list depends on, like a node's engine. */
  aside?: React.ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState("");
  const pos = usePointClamp(ref, x, y);
  const results = useMemo(
    () => (query.trim() ? runSearch(query) : []),
    [query, runSearch],
  );

  // Escape backs out a step at a time: it clears a search before closing, so a typo doesn't cost you the popup.
  useDismiss(ref, onClose, {
    onEscape: () => {
      if (!query) return false;
      setQuery("");
      return true;
    },
  });

  // Nothing to offer: an empty popup would just be a grey box you have to dismiss.
  if (!tree.length && !header && !aside) return null;

  const pick = (e: T) => {
    onPick(e);
    onClose();
  };

  return (
    <div
      ref={ref}
      style={{ left: pos.x, top: pos.y }}
      className="fixed z-(--k7-z-menu) flex items-start"
    >
      {aside}
      <div
        role="dialog"
        aria-label={menuLabel}
        className="menu flex min-w-[13rem] max-w-[min(26rem,calc(100vw-16px))] flex-col gap-1 rounded-(--k7-r-menu) p-1.5 text-sm"
      >
        <SearchList
          items={results}
          rank={null}
          query={query}
          onQueryChange={setQuery}
          onPick={(r) => r.entry !== undefined && pick(r.entry)}
          renderRow={(r) => <Leaf n={r} />}
          itemLabel={(r) => r.label}
          rowClassName="justify-between gap-4"
          emptyLabel={emptyLabel}
          limit={searchLimit}
          moreLabel={`first ${searchLimit} matches`}
          searchLabel={searchLabel ?? placeholder}
          placeholder={placeholder}
          before={header}
          autoFocus
          idle={
            tree.length ? (
              <Level nodes={tree} onPick={pick} label={menuLabel} />
            ) : (
              <EmptyRow>{emptyLabel}</EmptyRow>
            )
          }
        />
      </div>
    </div>
  );
}
