import * as React from "react";
import { cn } from "./lib/utils";

const SURFACES = {
  row: "row-group",
  quiet:
    "rounded-(--k7-r-rows) border border-(--k7-hairline-soft) bg-(--k7-card-quiet)",
  glass: "glass rounded-(--k7-r-rows)",
  float: "float rounded-(--k7-r-rows)",
  panel: "panel rounded-(--k7-r-panel)",
  dialog: "panel-dialog rounded-(--k7-r-dialog)",
  sheet: "panel-sheet rounded-t-(--k7-r-sheet)",
  menu: "menu rounded-(--k7-r-menu)",
} as const;

export type CardVariant = keyof typeof SURFACES;

// A surface: `row` (a row group, the default), or one of the canvas's glass, float and panel surfaces.
// No outer margin — spacing between cards is the parent's (a `gap`, a `space-y-*`).
const Card = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & { variant?: CardVariant }
>(({ className, variant = "row", ...props }, ref) => (
  <div
    ref={ref}
    className={cn(
      SURFACES[variant],
      "p-[18px] text-card-foreground",
      className,
    )}
    {...props}
  />
));
Card.displayName = "Card";

// Structural subcomponents (shadcn-style). The Card itself already carries the
// panel padding, so these only provide layout and typography — no extra inset.
const CardHeader = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn("mb-3 flex flex-col gap-1.5", className)}
    {...props}
  />
));
CardHeader.displayName = "CardHeader";

const CardTitle = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("type-headline", className)} {...props} />
));
CardTitle.displayName = "CardTitle";

const CardDescription = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn("type-secondary text-fg-2", className)}
    {...props}
  />
));
CardDescription.displayName = "CardDescription";

const CardAction = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("ml-auto self-start", className)} {...props} />
));
CardAction.displayName = "CardAction";

const CardContent = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn(className)} {...props} />
));
CardContent.displayName = "CardContent";

const CardFooter = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn("mt-3 flex items-center gap-2", className)}
    {...props}
  />
));
CardFooter.displayName = "CardFooter";

export {
  Card,
  CardAction,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
};
