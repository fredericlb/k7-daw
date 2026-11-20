import * as TabsPrimitive from "@radix-ui/react-tabs";
import * as React from "react";
import { useLozenge } from "./lib/lozenge";
import { cn } from "./lib/utils";

// A segmented control: a 40 px glass pill whose selected tab is a lozenge that slides under the labels.
const Tabs = TabsPrimitive.Root;

const TabsList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.List>
>(({ className, children, ...props }, forwarded) => {
  const own = React.useRef<HTMLDivElement | null>(null);
  const box = useLozenge(own, '[data-state="active"]');
  return (
    <TabsPrimitive.List
      ref={(el) => {
        own.current = el;
        if (typeof forwarded === "function") forwarded(el);
        else if (forwarded) forwarded.current = el;
      }}
      className={cn(
        "glass relative flex h-10 w-full gap-0.5 rounded-full p-[3px]",
        className,
      )}
      {...props}
    >
      {box ? (
        <span
          aria-hidden
          className="pointer-events-none absolute top-[3px] left-0 h-8 rounded-2xl bg-(--k7-on-fill-tab) shadow-(--k7-highlight) transition-[transform,width] duration-(--k7-dur-tab) ease-(--k7-ease-tab)"
          style={{ transform: `translateX(${box.x}px)`, width: box.w }}
        />
      ) : null}
      {children}
    </TabsPrimitive.List>
  );
});
TabsList.displayName = TabsPrimitive.List.displayName;

const TabsTrigger = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Trigger
    ref={ref}
    className={cn(
      "type-tab relative z-10 inline-flex h-8 flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-2xl px-3 text-foreground outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) disabled:pointer-events-none disabled:opacity-40 data-[state=active]:text-lavender",
      className,
    )}
    {...props}
  />
));
TabsTrigger.displayName = TabsPrimitive.Trigger.displayName;

const TabsContent = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn("outline-none", className)}
    {...props}
  />
));
TabsContent.displayName = TabsPrimitive.Content.displayName;

export { Tabs, TabsContent, TabsList, TabsTrigger };
