// dock-property.tsx — one of an editor's settings in its title bar, read as "name: value"; a tap opens a bubble above it
// holding the control that changes it (a knob, a switch…). Every editor shows its settings this one way (docs/editor-dock.md).
// What a reader gets wrong: the bubble holds the control alone; a knob already shows its value, and the bar shows it too.

import { Popover as PopoverPrimitive } from "radix-ui";
import * as React from "react";
import { Knob } from "./knob";
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
        <span className="type-mono-value text-fg-1 tabular-nums">
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
          className="menu z-(--k7-z-menu) flex min-w-16 flex-col items-center gap-2 rounded-(--k7-r-menu) p-3 text-fg-1 outline-hidden data-[state=open]:animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)]"
        >
          {children}
        </PopoverPrimitive.Content>
      </PopoverPrimitive.Portal>
    </PopoverPrimitive.Root>
  );
}

/** A knob in a title-bar property: a draft while it turns, one `onCommit` on release, `defaultValue` on a double-click. */
export function DockKnobProperty({
  name,
  value,
  min,
  max,
  step,
  defaultValue,
  format,
  onCommit,
  title,
  label,
}: {
  /** The setting's name as shown, lowercase; also the knob's accessible name. */
  name: string;
  value: number;
  min: number;
  max: number;
  step: number;
  defaultValue: number;
  format: (v: number) => string;
  onCommit: (v: number) => void;
  /** A hint on the knob's bubble. */
  title?: string;
  /** The knob's accessible name when it differs from `name`. */
  label?: string;
}) {
  const [draft, setDraft] = React.useState<number | null>(null);
  const shown = draft ?? value;
  return (
    <DockProperty name={name} value={format(shown)}>
      <div title={title}>
        <Knob
          value={shown}
          min={min}
          max={max}
          step={step}
          size={36}
          label={label ?? name}
          format={format}
          defaultValue={defaultValue}
          onValueChange={setDraft}
          onValueCommit={(v) => {
            setDraft(null);
            if (Math.abs(v - value) > 1e-6) onCommit(v);
          }}
        />
      </div>
    </DockProperty>
  );
}
