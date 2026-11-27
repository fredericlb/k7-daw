// choice.tsx — a radiogroup (or tablist) of 2 to 5 options: a segmented control on Pad and Desk, 44 px radio rows on Phone.
// The page picks the layout (it knows the device); Radix owns the arrows and the roles — RadioGroup, or Tabs for `role: "tab"`.

import { RadioGroup, Tabs } from "radix-ui";
import * as React from "react";
import { Check } from "./icons";
import { useLozenge } from "./lib/lozenge";
import { cn } from "./lib/utils";
import { RowButton, RowGroup } from "./row-group";

export interface ChoiceOption<T extends string> {
  value: T;
  label: string;
  icon?: React.ReactNode;
  /** The tooltip, where it says more than the label. */
  title?: string;
}

export function Choice<T extends string>({
  label,
  options,
  value,
  onValueChange,
  layout = "segmented",
  role = "radio",
  iconOnly = false,
  disabled = false,
  className,
}: {
  /** The group's accessible name ("Transport"). */
  label: string;
  options: ChoiceOption<T>[];
  /** "" when nothing is chosen yet. */
  value: T | "";
  onValueChange: (v: T) => void;
  layout?: "segmented" | "rows";
  /** "tab" draws a tablist (aria-selected); the default is a radiogroup. */
  role?: "radio" | "tab";
  /** Segmented only: the glyph alone, the word its accessible name and tooltip. */
  iconOnly?: boolean;
  disabled?: boolean;
  className?: string;
}) {
  const ref = React.useRef<HTMLDivElement | null>(null);
  const box = useLozenge(
    ref,
    role === "tab" ? '[aria-selected="true"]' : '[aria-checked="true"]',
  );

  if (layout === "rows")
    return (
      <RadioGroup.Root
        asChild
        aria-label={label}
        value={value}
        disabled={disabled}
        onValueChange={(v) => onValueChange(v as T)}
      >
        <RowGroup className={className}>
          {options.map((o) => (
            <RadioGroup.Item key={o.value} asChild value={o.value}>
              <RowButton
                selected={o.value === value}
                icon={o.icon}
                trailing={
                  o.value === value ? (
                    <Check className="size-4 text-lavender" strokeWidth={2.2} />
                  ) : null
                }
              >
                {o.label}
              </RowButton>
            </RadioGroup.Item>
          ))}
        </RowGroup>
      </RadioGroup.Root>
    );

  const frame = cn(
    "glass relative flex h-10 gap-0.5 rounded-full p-[3px]",
    className,
  );
  const pill = box ? (
    <span
      aria-hidden
      className="pointer-events-none absolute top-[3px] left-0 h-8 rounded-2xl bg-(--k7-on-fill-tab) shadow-(--k7-highlight) transition-[transform,width] duration-(--k7-dur-tab) ease-(--k7-ease-tab)"
      style={{ transform: `translateX(${box.x}px)`, width: box.w }}
    />
  ) : null;
  const itemClass = cn(
    "type-tab relative z-10 inline-flex h-8 min-w-0 cursor-pointer items-center justify-center gap-1.5 rounded-2xl text-fg-1 outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:cursor-default disabled:opacity-40 aria-checked:text-lavender aria-selected:text-lavender [&_svg]:size-4 [&_svg]:shrink-0",
    iconOnly ? "w-9 px-0" : "flex-1 px-3",
  );
  const body = (o: ChoiceOption<T>) => (
    <>
      {o.icon}
      {iconOnly ? null : <span className="truncate">{o.label}</span>}
    </>
  );
  const names = (o: ChoiceOption<T>) => ({
    title: o.title ?? (iconOnly ? o.label : undefined),
    "aria-label": iconOnly ? o.label : undefined,
  });
  if (role === "tab")
    return (
      <Tabs.Root value={value} onValueChange={(v) => onValueChange(v as T)}>
        <Tabs.List ref={ref} aria-label={label} className={frame}>
          {pill}
          {options.map((o) => (
            <Tabs.Trigger
              key={o.value}
              value={o.value}
              disabled={disabled}
              className={itemClass}
              {...names(o)}
            >
              {body(o)}
            </Tabs.Trigger>
          ))}
        </Tabs.List>
      </Tabs.Root>
    );
  return (
    <RadioGroup.Root
      ref={ref}
      aria-label={label}
      value={value}
      disabled={disabled}
      onValueChange={(v) => onValueChange(v as T)}
      className={frame}
    >
      {pill}
      {options.map((o) => (
        <RadioGroup.Item
          key={o.value}
          value={o.value}
          className={itemClass}
          {...names(o)}
        >
          {body(o)}
        </RadioGroup.Item>
      ))}
    </RadioGroup.Root>
  );
}
