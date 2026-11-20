import { cva, type VariantProps } from "class-variance-authority";
import { Toggle } from "radix-ui";
import type * as React from "react";
import { cn } from "./lib/utils";

// Four chips: anchor (a library chain), version (the newest is filled), filter (pressed when selected), status (git).
const chipVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40",
  {
    variants: {
      variant: {
        anchor:
          "type-mono-value h-[30px] rounded-[15px] border border-(--k7-hairline-strong) bg-(--k7-row) px-3 text-foreground",
        version:
          "type-secondary h-[30px] rounded-[15px] border border-(--k7-hairline-strong) px-3 font-bold text-foreground",
        filter:
          "type-secondary h-8 cursor-pointer rounded-2xl glass px-3.5 font-bold text-foreground hover:brightness-125 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight)",
        status:
          "type-badge h-6 rounded-xl border px-2.5 uppercase tracking-wide",
      },
      tone: {
        none: "",
        new: "border-(--k7-ok-line) bg-(--k7-ok-fill) text-ok",
        changed: "border-(--k7-warn-line) bg-(--k7-warn-fill) text-warn",
        removed: "border-(--k7-error-line) bg-(--k7-error-fill) text-error",
        renamed: "border-(--k7-hairline-strong) bg-(--k7-row) text-lavender",
        choose: "border-(--k7-warn-line) bg-(--k7-warn-fill) text-warn",
      },
    },
    defaultVariants: { variant: "anchor", tone: "none" },
  },
);

export interface ChipProps
  extends Omit<React.HTMLAttributes<HTMLElement>, "color">,
    VariantProps<typeof chipVariants> {
  /** anchor: a name that no library defines, drawn dashed in amber. */
  orphan?: boolean;
  /** version: the newest one is filled with the accent. */
  newest?: boolean;
  /** filter: the selected chip. */
  selected?: boolean;
  /** Render a button (a filter, an Upgrade target); the others are plain spans. */
  onClick?: React.MouseEventHandler<HTMLElement>;
}

export function Chip({
  variant,
  tone,
  orphan,
  newest,
  selected,
  className,
  onClick,
  ...props
}: ChipProps) {
  const cls = cn(
    chipVariants({ variant, tone }),
    variant === "anchor" &&
      orphan &&
      "border-dashed border-(--k7-warn-line) bg-(--k7-warn-fill) text-warn",
    variant === "version" &&
      newest &&
      "border-transparent bg-primary text-primary-foreground shadow-(--k7-highlight)",
    className,
  );
  if (variant === "filter")
    return (
      <Toggle.Root
        className={cls}
        pressed={!!selected}
        onClick={onClick}
        {...(props as React.ComponentProps<typeof Toggle.Root>)}
      />
    );
  if (onClick)
    return (
      <button
        type="button"
        className={cls}
        onClick={onClick}
        {...(props as React.ButtonHTMLAttributes<HTMLButtonElement>)}
      />
    );
  return <span className={cls} {...props} />;
}
