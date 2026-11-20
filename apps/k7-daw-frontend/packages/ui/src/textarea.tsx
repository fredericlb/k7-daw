import * as React from "react";
import { cn } from "./lib/utils";

const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.TextareaHTMLAttributes<HTMLTextAreaElement>
>(({ className, ...props }, ref) => (
  <textarea
    ref={ref}
    className={cn(
      "type-body flex min-h-20 w-full resize-y rounded-[14px] border border-(--k7-input-line) bg-(--k7-input-fill) px-3.5 py-2.5 text-foreground transition-colors placeholder:text-fg-3 focus-visible:border-(--k7-focus-line) focus-visible:bg-(--k7-input-fill-focus) focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40",
      className,
    )}
    {...props}
  />
));
Textarea.displayName = "Textarea";

export { Textarea };
