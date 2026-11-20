// choice.tsx — a radiogroup of 2 to 4 options: a segmented control on Pad and Desk, 44 px radio rows on Phone.
// The page picks the layout (it knows the device); Radix RadioGroup owns the arrows and the roles in both.

import { RadioGroup } from "radix-ui";
import * as React from "react";
import { Check } from "./icons";
import { useLozenge } from "./lib/lozenge";
import { cn } from "./lib/utils";
import { RowButton, RowGroup } from "./row-group";

export interface ChoiceOption<T extends string> {
  value: T;
  label: string;
  icon?: React.ReactNode;
}

export function Choice<T extends string>({
  label,
  options,
  value,
  onValueChange,
  layout = "segmented",
  className,
}: {
  /** The group's accessible name ("Transport"). */
  label: string;
  options: ChoiceOption<T>[];
  value: T;
  onValueChange: (v: T) => void;
  layout?: "segmented" | "rows";
  className?: string;
}) {
  const ref = React.useRef<HTMLDivElement | null>(null);
  const box = useLozenge(ref, '[aria-checked="true"]');

  if (layout === "rows")
    return (
      <RadioGroup.Root
        asChild
        aria-label={label}
        value={value}
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

  return (
    <RadioGroup.Root
      ref={ref}
      aria-label={label}
      value={value}
      onValueChange={(v) => onValueChange(v as T)}
      className={cn(
        "glass relative flex h-10 gap-0.5 rounded-full p-[3px]",
        className,
      )}
    >
      {box ? (
        <span
          aria-hidden
          className="pointer-events-none absolute top-[3px] left-0 h-8 rounded-2xl bg-(--k7-on-fill-tab) shadow-(--k7-highlight) transition-[transform,width] duration-(--k7-dur-tab) ease-(--k7-ease-tab)"
          style={{ transform: `translateX(${box.x}px)`, width: box.w }}
        />
      ) : null}
      {options.map((o) => (
        <RadioGroup.Item
          key={o.value}
          value={o.value}
          className="type-tab relative z-10 inline-flex h-8 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-2xl px-3 text-foreground outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) aria-checked:text-lavender [&_svg]:size-4"
        >
          {o.icon}
          {o.label}
        </RadioGroup.Item>
      ))}
    </RadioGroup.Root>
  );
}
