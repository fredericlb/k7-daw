import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import type * as React from "react";
import { cn } from "./lib/utils";

// `count` is the 22 px badge (`count-sm` 20 px on a corner), `warn` the 30 px header chip, `amber` its alias.
const badgeVariants = cva(
  "inline-flex w-fit shrink-0 items-center justify-center gap-1 whitespace-nowrap rounded-full px-2 py-0.5 text-[11px] transition-colors [a&]:cursor-pointer [button&]:cursor-pointer",
  {
    variants: {
      variant: {
        default: "bg-secondary text-muted-foreground tabular-nums",
        amber: "bg-amber/15 text-amber tabular-nums",
        rec: "bg-error/15 text-error tabular-nums",
        tag: "border border-primary/35 bg-primary/20 font-semibold text-primary-text",
        count:
          "type-badge h-[22px] min-w-[22px] rounded-[11px] bg-primary px-1.5 py-0 text-primary-foreground tabular-nums",
        "count-sm":
          "type-badge h-5 min-w-5 rounded-[10px] bg-primary px-1.5 py-0 text-primary-foreground tabular-nums",
        warn: "type-secondary h-[30px] gap-1.5 rounded-[15px] border border-(--k7-warn-line) bg-(--k7-warn-fill) px-2.5 py-0 font-bold text-warn tabular-nums",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {
  asChild?: boolean;
}

function Badge({ className, variant, asChild = false, ...props }: BadgeProps) {
  const Comp = asChild ? Slot : "span";
  return (
    <Comp className={cn(badgeVariants({ variant }), className)} {...props} />
  );
}

export { Badge, badgeVariants };
