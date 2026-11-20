// picker.tsx — a 56 px button showing the current item and its path; a list with a search field opens under it.
// For a long list of things with a name and a place (modules, files); a tree stays in the app's TreePicker.

import * as React from "react";
import { Check, ChevronDown, Search } from "./icons";
import { cn } from "./lib/utils";

export interface PickerItem {
  value: string;
  label: string;
  /** A path or kind under the label, mono. */
  detail?: string;
}

export function Picker({
  items,
  value,
  onValueChange,
  label,
  searchLabel,
  placeholder,
  emptyLabel,
  className,
}: {
  items: PickerItem[];
  value: string | null;
  onValueChange: (v: string) => void;
  /** The button's accessible name ("Playlist source"). */
  label: string;
  searchLabel: string;
  placeholder?: string;
  /** Shown when the search matches nothing. */
  emptyLabel: string;
  className?: string;
}) {
  const [open, setOpen] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const root = React.useRef<HTMLDivElement | null>(null);
  const id = React.useId();
  const current = items.find((i) => i.value === value);
  const q = query.trim().toLowerCase();
  const shown = q
    ? items.filter((i) =>
        `${i.label} ${i.detail ?? ""}`.toLowerCase().includes(q),
      )
    : items;
  const close = () => {
    setOpen(false);
    setQuery("");
  };

  return (
    <div
      ref={root}
      className={cn("relative", className)}
      onBlur={(e) => {
        if (!root.current?.contains(e.relatedTarget as Node | null)) close();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape" && open) {
          e.stopPropagation();
          close();
        }
      }}
    >
      <button
        type="button"
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen((o) => !o)}
        className="glass flex h-14 w-full cursor-pointer items-center gap-3 rounded-[18px] px-3.5 text-left outline-none focus-visible:shadow-(--k7-focus-ring)"
      >
        <span className="min-w-0 flex-1">
          <span className="type-body block truncate text-foreground">
            {current?.label ?? placeholder}
          </span>
          {current?.detail ? (
            <span className="type-mono-value block truncate text-fg-3">
              {current.detail}
            </span>
          ) : null}
        </span>
        <ChevronDown className="size-5 shrink-0 text-fg-2" strokeWidth={2.2} />
      </button>
      {open ? (
        <div
          id={id}
          className="menu absolute inset-x-0 top-[calc(100%+6px)] z-(--k7-z-menu) flex max-h-72 flex-col gap-1 rounded-(--k7-r-menu) p-1.5 animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)]"
        >
          <div className="relative">
            <Search
              aria-hidden
              className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-fg-3"
            />
            <input
              // biome-ignore lint/a11y/noAutofocus: the list opens for a search, so the field takes the keys
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              aria-label={searchLabel}
              placeholder={searchLabel}
              className="type-body h-11 w-full rounded-xl border border-(--k7-input-line) bg-(--k7-input-fill) pr-3 pl-9 text-foreground outline-none placeholder:text-fg-3 focus:border-(--k7-focus-line)"
            />
          </div>
          <div
            role="listbox"
            aria-label={label}
            className="min-h-0 overflow-y-auto"
          >
            {shown.map((i) => (
              <div key={i.value} role="presentation">
                <button
                  type="button"
                  role="option"
                  aria-selected={i.value === value}
                  onClick={() => {
                    onValueChange(i.value);
                    close();
                  }}
                  className="type-body flex min-h-11 w-full cursor-pointer items-center gap-2 rounded-xl px-3 text-left text-foreground outline-none hover:bg-on-fill focus-visible:bg-on-fill aria-selected:text-lavender"
                >
                  <span className="min-w-0 flex-1">
                    <span className="block truncate">{i.label}</span>
                    {i.detail ? (
                      <span className="type-mono-value block truncate text-fg-3">
                        {i.detail}
                      </span>
                    ) : null}
                  </span>
                  {i.value === value ? (
                    <Check className="size-4 shrink-0" strokeWidth={2.2} />
                  ) : null}
                </button>
              </div>
            ))}
            {shown.length === 0 ? (
              <div className="type-secondary px-3 py-3 text-fg-3">
                {emptyLabel}
              </div>
            ) : null}
          </div>
        </div>
      ) : null}
    </div>
  );
}
