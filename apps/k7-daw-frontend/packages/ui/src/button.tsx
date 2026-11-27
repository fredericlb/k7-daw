import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";
import { reasonOf, reasonPress } from "./disabled-reason";
import { cn } from "./lib/utils";

// A control is a pill (radius = height / 2). `glass` and `chip` carry a pressed state through aria-pressed.
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-full type-label cursor-pointer transition-colors outline-none focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40 aria-disabled:opacity-40 [&_svg]:pointer-events-none [&_svg]:shrink-0 desk:[&_svg.size-5]:size-[18px]",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-on-accent shadow-(--k7-highlight) hover:bg-primary-hover",
        secondary: "glass text-fg-1 hover:brightness-125",
        outline:
          "border border-(--k7-hairline-strong) bg-transparent text-fg-1 hover:bg-on-fill",
        ghost: "text-fg-1 hover:bg-on-fill",
        glass:
          "glass text-fg-1 hover:brightness-125 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight)",
        chip: "glass type-secondary font-bold text-fg-1 hover:brightness-125 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight)",
        destructive: "bg-error text-ink hover:brightness-95",
        link: "h-auto rounded-none p-0 text-primary-text tabular-nums underline-offset-4 hover:underline",
      },
      size: {
        default: "h-11 px-[18px] desk:h-8 desk:px-3.5",
        xs: "h-8 gap-1 px-3 text-[13px]",
        sm: "h-9 px-3.5 desk:h-8",
        lg: "h-12 px-7",
        icon: "size-11 desk:size-8",
        round: "size-11 desk:size-8",
        "icon-xs": "size-8",
        "icon-sm": "size-9 desk:size-8",
        // A title-bar glyph button: 36 px on every device, as the bar around it is.
        "icon-md": "size-9",
        "icon-2xs": "size-6",
        // A compact text button of a row that is 28 px tall (the timeline's corner).
        "2xs": "h-6 gap-1 px-3 text-[11px]",
        // A text value pressed in a bar (the tempo): 36 px on every device.
        bar: "h-9 px-2",
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
  /** Why it is disabled: it then stays pressable (aria-disabled) and a press shows this in a toast. */
  disabledReason?: string;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant, size, asChild = false, disabledReason, ...props },
    ref,
  ) => {
    const Comp = asChild ? Slot : "button";
    // A disabled button with a reason stays pressable (aria-disabled) and shows the reason on press.
    const reason = reasonOf(props.disabled, disabledReason);
    if (reason) {
      const { disabled: _d, onClick: _c, ...rest } = props;
      return (
        <Comp
          className={cn(buttonVariants({ variant, size, className }))}
          ref={ref}
          aria-disabled
          title={props.title ?? reason}
          {...rest}
          onClick={reasonPress(reason)}
        />
      );
    }
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
