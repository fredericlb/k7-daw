import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";
import { cn } from "./lib/utils";

// A control is a pill (radius = height / 2). `glass` and `chip` carry a pressed state through aria-pressed.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-full type-label cursor-pointer transition-colors outline-none focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40 [&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow-(--k7-highlight) hover:bg-primary-hover",
        secondary: "glass text-foreground hover:brightness-125",
        outline:
          "border border-(--k7-hairline-strong) bg-transparent text-foreground hover:bg-on-fill",
        ghost: "text-foreground hover:bg-on-fill",
        glass:
          "glass text-foreground hover:brightness-125 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight)",
        chip: "glass type-secondary font-bold text-foreground hover:brightness-125 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight)",
        destructive: "bg-error text-ink hover:brightness-95",
        link: "h-auto rounded-none p-0 text-primary-text tabular-nums underline-offset-4 hover:underline",
      },
      size: {
        default: "h-11 px-[18px]",
        xs: "h-8 gap-1 px-3 text-[13px]",
        sm: "h-9 px-3.5",
        lg: "h-12 px-7",
        icon: "size-11",
        round: "size-11",
        "icon-xs": "size-8",
        "icon-sm": "size-9",
        "icon-lg": "size-12",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button";
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
