import type * as React from "react";
import { cn } from "./lib/utils";

// The caps line over a block of rows or a group of settings; one colour, spacing from the caller.
export function SectionHeading({
  as: Tag = "h3",
  className,
  ...props
}: React.HTMLAttributes<HTMLHeadingElement> & { as?: "h2" | "h3" }) {
  return <Tag className={cn("type-caps text-fg-3", className)} {...props} />;
}
