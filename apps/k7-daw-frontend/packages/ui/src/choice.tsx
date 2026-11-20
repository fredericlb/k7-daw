// choice.tsx — a radiogroup of 2 to 4 options: a segmented control on Pad and Desk, 44 px radio rows on Phone.
// The page picks the layout (it knows the device); arrow keys move the choice in both.

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
  const move = (e: React.KeyboardEvent) => {
    const step =
      e.key === "ArrowRight" || e.key === "ArrowDown"
        ? 1
        : e.key === "ArrowLeft" || e.key === "ArrowUp"
          ? -1
          : 0;
    if (!step) return;
    e.preventDefault();
    const i = options.findIndex((o) => o.value === value);
    const next = options[(i + step + options.length) % options.length];
    if (next) onValueChange(next.value);
  };

  if (layout === "rows")
    return (
      <RowGroup
        role="radiogroup"
        aria-label={label}
        onKeyDown={move}
        className={className}
      >
        {options.map((o) => (
          <RowButton
            key={o.value}
            role="radio"
            aria-checked={o.value === value}
            selected={o.value === value}
            icon={o.icon}
            trailing={
              o.value === value ? (
                <Check className="size-4 text-lavender" strokeWidth={2.2} />
              ) : null
            }
            onClick={() => onValueChange(o.value)}
          >
            {o.label}
          </RowButton>
        ))}
      </RowGroup>
    );

  return (
    <div
      ref={ref}
      role="radiogroup"
      aria-label={label}
      onKeyDown={move}
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
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={o.value === value}
          tabIndex={o.value === value ? 0 : -1}
          onClick={() => onValueChange(o.value)}
          className="type-tab relative z-10 inline-flex h-8 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-2xl px-3 text-foreground outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) aria-checked:text-lavender [&_svg]:size-4"
        >
          {o.icon}
          {o.label}
        </button>
      ))}
    </div>
  );
}
