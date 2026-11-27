import * as DropdownMenuPrimitive from "@radix-ui/react-dropdown-menu";
import * as React from "react";
import { reasonOf, showReason } from "./disabled-reason";
import { Check, ChevronRight } from "./icons";
import { cn } from "./lib/utils";

const DropdownMenu = DropdownMenuPrimitive.Root;
const DropdownMenuTrigger = DropdownMenuPrimitive.Trigger;
const DropdownMenuPortal = DropdownMenuPrimitive.Portal;

const DropdownMenuContent = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Content>
>(({ className, sideOffset = 6, ...props }, ref) => (
  <DropdownMenuPrimitive.Portal>
    <DropdownMenuPrimitive.Content
      ref={ref}
      sideOffset={sideOffset}
      className={cn(
        "menu z-(--k7-z-menu) min-w-[10rem] overflow-hidden rounded-(--k7-r-menu) p-1.5 text-fg-1",
        "origin-(--radix-dropdown-menu-content-transform-origin) data-[state=open]:animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)] data-[state=closed]:animate-out data-[state=closed]:fade-out-0",
        className,
      )}
      {...props}
    />
  </DropdownMenuPrimitive.Portal>
));
DropdownMenuContent.displayName = DropdownMenuPrimitive.Content.displayName;

const DropdownMenuItem = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Item> & {
    /** Why it is disabled: it then stays selectable (aria-disabled) and choosing it shows this in a toast. */
    disabledReason?: string;
  }
>(({ className, disabledReason, ...props }, ref) => {
  // A disabled item with a reason stays selectable (aria-disabled); choosing it shows the reason.
  const reason = reasonOf(props.disabled, disabledReason);
  const { disabled: _d, onSelect: _s, ...rest } = props;
  return (
    <DropdownMenuPrimitive.Item
      ref={ref}
      className={cn(
        "type-body relative flex min-h-11 cursor-pointer desk:min-h-8 desk:py-1 select-none items-center gap-2.5 rounded-xl px-3 py-2 outline-none transition-colors focus:bg-on-fill data-[disabled]:pointer-events-none data-[disabled]:opacity-40 aria-disabled:opacity-40 [&_svg]:size-[19px] [&_svg]:text-primary-text",
        className,
      )}
      {...(reason
        ? {
            ...rest,
            title: props.title ?? reason,
            "aria-disabled": true,
            onSelect: () => showReason(reason),
          }
        : props)}
    />
  );
});
DropdownMenuItem.displayName = DropdownMenuPrimitive.Item.displayName;

// A row that toggles: it stays open on a pick, since choosing several takes several picks.
const DropdownMenuCheckboxItem = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.CheckboxItem>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.CheckboxItem>
>(({ className, children, onSelect, ...props }, ref) => (
  <DropdownMenuPrimitive.CheckboxItem
    ref={ref}
    onSelect={(e) => {
      e.preventDefault();
      onSelect?.(e);
    }}
    className={cn(
      "type-body relative flex min-h-11 cursor-pointer select-none items-center gap-2.5 rounded-xl px-3 py-2 outline-none transition-colors focus:bg-on-fill data-[disabled]:pointer-events-none data-[disabled]:opacity-40 desk:min-h-8 desk:py-1",
      className,
    )}
    {...props}
  >
    <span className="flex size-4 shrink-0 items-center justify-center rounded border border-(--k7-hairline-strong)">
      <DropdownMenuPrimitive.ItemIndicator>
        <Check className="size-3" strokeWidth={3} />
      </DropdownMenuPrimitive.ItemIndicator>
    </span>
    {children}
  </DropdownMenuPrimitive.CheckboxItem>
));
DropdownMenuCheckboxItem.displayName =
  DropdownMenuPrimitive.CheckboxItem.displayName;

const DropdownMenuLabel = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Label>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Label>
>(({ className, ...props }, ref) => (
  <DropdownMenuPrimitive.Label
    ref={ref}
    className={cn("type-caps px-3 py-1.5 text-fg-3", className)}
    {...props}
  />
));
DropdownMenuLabel.displayName = DropdownMenuPrimitive.Label.displayName;

const DropdownMenuSeparator = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <DropdownMenuPrimitive.Separator
    ref={ref}
    className={cn("-mx-1.5 my-1 h-px bg-(--k7-hairline-row)", className)}
    {...props}
  />
));
DropdownMenuSeparator.displayName = DropdownMenuPrimitive.Separator.displayName;

const DropdownMenuSub = DropdownMenuPrimitive.Sub;

// A submenu's row: an item that opens its menu beside it, the chevron saying so.
const DropdownMenuSubTrigger = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.SubTrigger>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.SubTrigger>
>(({ className, children, ...props }, ref) => (
  <DropdownMenuPrimitive.SubTrigger
    ref={ref}
    className={cn(
      "type-body relative flex min-h-11 cursor-pointer desk:min-h-8 desk:py-1 select-none items-center gap-2.5 rounded-xl px-3 py-2 outline-none transition-colors focus:bg-on-fill data-[state=open]:bg-on-fill data-[disabled]:pointer-events-none data-[disabled]:opacity-40 [&_svg]:size-[19px] [&_svg]:text-primary-text",
      className,
    )}
    {...props}
  >
    {children}
    <ChevronRight className="ml-auto" />
  </DropdownMenuPrimitive.SubTrigger>
));
DropdownMenuSubTrigger.displayName =
  DropdownMenuPrimitive.SubTrigger.displayName;

const DropdownMenuSubContent = React.forwardRef<
  React.ElementRef<typeof DropdownMenuPrimitive.SubContent>,
  React.ComponentPropsWithoutRef<typeof DropdownMenuPrimitive.SubContent>
>(({ className, sideOffset = 6, ...props }, ref) => (
  <DropdownMenuPrimitive.Portal>
    <DropdownMenuPrimitive.SubContent
      ref={ref}
      sideOffset={sideOffset}
      className={cn(
        "menu z-(--k7-z-menu) min-w-[10rem] overflow-hidden rounded-(--k7-r-menu) p-1.5 text-fg-1",
        "origin-(--radix-dropdown-menu-content-transform-origin) data-[state=open]:animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)] data-[state=closed]:animate-out data-[state=closed]:fade-out-0",
        className,
      )}
      {...props}
    />
  </DropdownMenuPrimitive.Portal>
));
DropdownMenuSubContent.displayName =
  DropdownMenuPrimitive.SubContent.displayName;

export {
  DropdownMenu,
  DropdownMenuCheckboxItem,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuPortal,
  DropdownMenuSeparator,
  DropdownMenuSub,
  DropdownMenuSubContent,
  DropdownMenuSubTrigger,
  DropdownMenuTrigger,
};
