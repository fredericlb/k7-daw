import * as React from "react";
import { reasonOf, reasonPress } from "./disabled-reason";
import { cn } from "./lib/utils";

// A glass pill of icon buttons (44 x 36 each, padding 3): the header's git, pane and panel capsules and the bar's transport.
export const Capsule = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, role = "group", ...props }, ref) => (
  <div
    ref={ref}
    role={role}
    className={cn(
      "glass inline-flex h-11 items-center gap-0.5 rounded-full p-[3px]",
      className,
    )}
    {...props}
  />
));
Capsule.displayName = "Capsule";

// `pressed` shows the on state (lavender, on-fill, inset) and is announced through aria-pressed.
export const CapsuleButton = React.forwardRef<
  HTMLButtonElement,
  React.ButtonHTMLAttributes<HTMLButtonElement> & { pressed?: boolean }
>(({ className, pressed, type = "button", ...props }, ref) => {
  // A disabled button with a reason stays pressable (aria-disabled) and shows the reason on press.
  const reason = reasonOf(props.disabled, props.title);
  const { disabled: _d, onClick: _c, ...rest } = props;
  return (
    <button
      ref={ref}
      type={type}
      aria-pressed={pressed}
      className={cn(
        "type-label inline-flex h-9 min-w-11 cursor-pointer items-center justify-center gap-1.5 rounded-[18px] px-2.5 text-foreground outline-none transition-colors hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40 aria-disabled:opacity-40 aria-pressed:bg-on-fill aria-pressed:text-lavender aria-pressed:shadow-(--k7-highlight) [&_svg]:size-5 [&_svg]:shrink-0",
        className,
      )}
      {...(reason
        ? { ...rest, "aria-disabled": true, onClick: reasonPress(reason) }
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
