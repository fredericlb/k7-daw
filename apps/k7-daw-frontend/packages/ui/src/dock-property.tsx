// dock-property.tsx — one of an editor's settings in its title bar, read as "name: value"; a tap opens a bubble above it
// holding the control that changes it (a knob, a switch…). Every editor shows its settings this one way (docs/editor-dock.md).
// What a reader gets wrong: the bubble's control is the caller's, so a value that commits on release still does.

import { Popover as PopoverPrimitive } from "radix-ui";
import type * as React from "react";
import { cn } from "./lib/utils";

export function DockProperty({
  name,
  value,
  children,
  open,
  onOpenChange,
  disabled,
  className,
}: {
  /** The setting's name as shown, lowercase ("gain"). */
  name: string;
  /** Its value as shown ("0.0 dB", "off"). */
  value: string;
  /** The control the bubble holds. */
  children: React.ReactNode;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <PopoverPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <PopoverPrimitive.Trigger
        disabled={disabled}
        data-slot="dock-property"
        className={cn(
          "type-secondary inline-flex h-8 shrink-0 cursor-pointer items-center gap-1 whitespace-nowrap rounded-2xl px-2.5 outline-none hover:bg-(--k7-row) focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40 data-[state=open]:bg-(--k7-row)",
          className,
        )}
      >
        <span className="text-fg-2">{name}:</span>
        <span className="type-mono-value text-foreground tabular-nums">
          {value}
        </span>
      </PopoverPrimitive.Trigger>
      <PopoverPrimitive.Portal>
        <PopoverPrimitive.Content
          side="top"
          align="center"
          sideOffset={8}
          aria-label={name}
          data-slot="dock-property-bubble"
          className="menu z-(--k7-z-menu) flex min-w-16 flex-col items-center gap-2 rounded-(--k7-r-menu) p-3 text-popover-foreground outline-hidden data-[state=open]:animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)]"
        >
          {children}
          <span className="type-mono-value text-fg-2 tabular-nums">
            {value}
          </span>
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}
