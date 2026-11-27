// picker.tsx — a 56 px button showing the current item and its path; a list with a search field opens under it.
// Radix Popover places it and owns Escape and outside clicks; Radix Select cannot hold the search field (docs/radix.md).
// For a long list of things with a name and a place (modules, files); a tree stays in the app's TreePicker.

import { Popover } from "radix-ui";
import * as React from "react";
import { Check, ChevronDown } from "./icons";
import { cn } from "./lib/utils";
import { SearchList } from "./search-list";

export interface PickerItem {
  value: string;
  label: string;
  /** A path or kind under the label, mono. */
  detail?: string;
  /** The heading it is listed under; consecutive items with one group share it. */
  group?: string;
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
  trigger,
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
  /** What the button shows instead of the 56 px item card, for a compact place such as a title bar; `className` then styles it alone. */
  trigger?: React.ReactNode;
}) {
  const [open, setOpen] = React.useState(false);
  const current = items.find((i) => i.value === value);
  const change = (o: boolean) => setOpen(o);

  return (
    <Popover.Root open={open} onOpenChange={change}>
      <Popover.Trigger
        aria-label={label}
        aria-haspopup="listbox"
        className={
          trigger
            ? className
            : cn(
                "glass flex h-14 w-full cursor-pointer items-center gap-3 rounded-[18px] px-3.5 text-left outline-none focus-visible:shadow-(--k7-focus-ring)",
                className,
              )
        }
      >
        {trigger ?? (
          <>
            <span className="min-w-0 flex-1">
              <span className="type-body block truncate text-fg-1">
                {current?.label ?? placeholder}
              </span>
              {current?.detail ? (
                <span className="type-mono-value block truncate text-fg-3">
                  {current.detail}
                </span>
              ) : null}
            </span>
            <ChevronDown
              className="size-5 shrink-0 text-fg-2"
              strokeWidth={2.2}
            />
          </>
        )}
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content
          role="group"
          aria-label={label}
          align="start"
          sideOffset={6}
          collisionPadding={8}
          onEscapeKeyDown={(e) => e.stopPropagation()}
          className="menu z-(--k7-z-menu) flex max-h-[min(18rem,var(--radix-popover-content-available-height))] w-(--radix-popover-trigger-width) min-w-56 origin-(--radix-popover-content-transform-origin) flex-col gap-1 rounded-(--k7-r-menu) p-1.5 outline-none data-[state=open]:animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)]"
        >
          <SearchList
            items={items}
            text={(i) => `${i.label} ${i.detail ?? ""} ${i.group ?? ""}`}
            onPick={(i) => {
              onValueChange(i.value);
              change(false);
            }}
            isSelected={(i) => i.value === value}
            heading={(i, prev) =>
              i.group && i.group !== prev?.group ? (
                <div className="type-label px-3 pt-2 pb-1 text-fg-3">
                  {i.group}
                </div>
              ) : null
            }
            renderRow={(i) => (
              <>
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
              </>
            )}
            emptyLabel={emptyLabel}
            searchLabel={searchLabel}
            className="max-h-[inherit]"
          />
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
