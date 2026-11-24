import * as React from "react";
import { reasonOf, reasonPress } from "./disabled-reason";
import { cn } from "./lib/utils";

// A glass pill of icon buttons (44 x 36 each, padding 3; 32 tall on a desk): the header's git, pane and panel capsules and the bar's transport.
export const Capsule = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, role = "group", ...props }, ref) => (
  <div
    ref={ref}
    role={role}
    className={cn(
      "glass inline-flex h-11 items-center desk:h-8 gap-0.5 rounded-full p-[3px]",
      className,
    )}
    {...props}
  />
));
Capsule.displayName = "Capsule";

// `pressed` shows the on state (lavender, on-fill, inset) and is announced through aria-pressed.
export const CapsuleButton = React.forwardRef<
  HTMLButtonElement,
  React.ButtonHTMLAttributes<HTMLButtonElement> & {
    pressed?: boolean;
    /** Why it is disabled: it then stays pressable and a press shows this in a toast. */
    disabledReason?: string;
  }
>(({ className, pressed, type = "button", disabledReason, ...props }, ref) => {
  // A disabled button with a reason stays pressable (aria-disabled) and shows the reason on press.
  const reason = reasonOf(props.disabled, disabledReason);
  const { disabled: _d, onClick: _c, ...rest } = props;
  return (
    <button
      ref={ref}
      type={type}
      aria-pressed={pressed}
      className={cn(
        "type-label inline-flex h-9 min-w-11 cursor-pointer desk:h-6 desk:min-w-8 desk:rounded-xl items-center justify-center gap-1.5 rounded-[18px] px-2.5 text-foreground outline-none transition-colors hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40 aria-disabled:opacity-40 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight) [&_svg]:size-5 [&_svg]:shrink-0",
        className,
      )}
      {...(reason
        ? {
            ...rest,
            title: props.title ?? reason,
            "aria-disabled": true,
            onClick: reasonPress(reason),
          }
        : props)}
    />
  );
});
CapsuleButton.displayName = "CapsuleButton";

// A hairline between two groups of a capsule.
export function CapsuleDivider({ className }: { className?: string }) {
  return (
    <span
      aria-hidden
      className={cn("mx-0.5 h-5 w-px bg-(--k7-hairline-strong)", className)}
    />
  );
}
