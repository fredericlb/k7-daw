import * as React from "react";
import { cn } from "./lib/utils";

const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, type, ...props }, ref) => (
  <input
    type={type}
    ref={ref}
    className={cn(
      // flex-1/min-w-0 so inputs share .row flex lines like the legacy CSS did
      "type-body flex h-11 w-full desk:h-8 flex-1 min-w-0 rounded-[14px] border border-(--k7-input-line) bg-(--k7-input-fill) px-3.5 text-foreground transition-colors placeholder:text-fg-3 focus-visible:border-(--k7-focus-line) focus-visible:bg-(--k7-input-fill-focus) focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40",
      "file:me-2 file:h-full file:cursor-pointer file:rounded-full file:border-0 file:bg-secondary file:px-3 file:text-xs file:font-bold file:text-foreground",
      className,
    )}
    {...props}
  />
));
Input.displayName = "Input";

export { Input };
