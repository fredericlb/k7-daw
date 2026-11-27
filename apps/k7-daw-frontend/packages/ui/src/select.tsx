import { Select as SelectPrimitive } from "radix-ui";
import type * as React from "react";
import { CheckIcon, ChevronDownIcon, ChevronUpIcon } from "./icons";

import { cn } from "./lib/utils";

function Select({
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Root>) {
  return <SelectPrimitive.Root data-slot="select" {...props} />;
}

function SelectGroup({
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Group>) {
  return <SelectPrimitive.Group data-slot="select-group" {...props} />;
}

function SelectValue({
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Value>) {
  return <SelectPrimitive.Value data-slot="select-value" {...props} />;
}

function SelectTrigger({
  className,
  size = "default",
  children,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Trigger> & {
  size?: "sm" | "default";
}) {
  return (
    <SelectPrimitive.Trigger
      data-slot="select-trigger"
      data-size={size}
      className={cn(
        "type-body flex w-fit items-center justify-between gap-2 rounded-[14px] border border-(--k7-input-line) bg-(--k7-input-fill) px-3.5 whitespace-nowrap transition-colors outline-none focus-visible:border-(--k7-focus-line) focus-visible:shadow-(--k7-focus-ring) disabled:cursor-not-allowed disabled:opacity-40 aria-invalid:border-error data-[placeholder]:text-fg-3 data-[size=default]:h-11 desk:data-[size=default]:h-8 desk:data-[size=sm]:h-8 data-[size=sm]:h-9 data-[size=sm]:rounded-[12px] *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-fg-2",
        className,
      )}
      {...props}
    >
      {children}
      <SelectPrimitive.Icon asChild>
        <ChevronDownIcon className="size-4 opacity-50" />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
}

function SelectContent({
  className,
  children,
  position = "item-aligned",
  align = "center",
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Content>) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        data-slot="select-content"
        className={cn(
          "menu relative z-(--k7-z-menu) max-h-(--radix-select-content-available-height) min-w-[8rem] origin-(--radix-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-(--k7-r-menu) text-fg-1 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:animate-[k7-menu-in_var(--k7-dur-menu)_var(--k7-ease-spring)]",
          position === "popper" &&
            "data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 data-[side=right]:translate-x-1 data-[side=top]:-translate-y-1",
          className,
        )}
        position={position}
        align={align}
        {...props}
      >
        <SelectScrollUpButton />
        <SelectPrimitive.Viewport
          className={cn(
            "p-1",
            position === "popper" &&
              "h-[var(--radix-select-trigger-height)] w-full min-w-[var(--radix-select-trigger-width)] scroll-my-1",
          )}
        >
          {children}
        </SelectPrimitive.Viewport>
        <SelectScrollDownButton />
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
}

function SelectLabel({
  className,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Label>) {
  return (
    <SelectPrimitive.Label
      data-slot="select-label"
      className={cn("type-caps px-3 py-1.5 text-fg-3", className)}
      {...props}
    />
  );
}

function SelectItem({
  className,
  children,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Item>) {
  return (
    <SelectPrimitive.Item
      data-slot="select-item"
      className={cn(
        "type-body relative flex min-h-11 w-full cursor-default desk:min-h-8 desk:py-1 items-center gap-2 rounded-xl py-2 pr-9 pl-3 outline-hidden select-none focus:bg-on-fill data-[state=checked]:text-lavender data-[disabled]:pointer-events-none data-[disabled]:opacity-40 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-fg-2 *:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2",
        className,
      )}
      {...props}
    >
      <span
        data-slot="select-item-indicator"
        className="absolute right-2 flex size-3.5 items-center justify-center"
      >
        <SelectPrimitive.ItemIndicator>
          <CheckIcon className="size-4" />
        </SelectPrimitive.ItemIndicator>
      </span>
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
    </SelectPrimitive.Item>
  );
}

function SelectSeparator({
  className,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.Separator>) {
  return (
    <SelectPrimitive.Separator
      data-slot="select-separator"
      className={cn("pointer-events-none -mx-1 my-1 h-px bg-hairline", className)}
      {...props}
    />
  );
}

function SelectScrollUpButton({
  className,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.ScrollUpButton>) {
  return (
    <SelectPrimitive.ScrollUpButton
      data-slot="select-scroll-up-button"
      className={cn(
        "flex cursor-default items-center justify-center py-1",
        className,
      )}
      {...props}
    >
      <ChevronUpIcon className="size-4" />
    </SelectPrimitive.ScrollUpButton>
  );
}

function SelectScrollDownButton({
  className,
  ...props
}: React.ComponentProps<typeof SelectPrimitive.ScrollDownButton>) {
  return (
    <SelectPrimitive.ScrollDownButton
      data-slot="select-scroll-down-button"
      className={cn(
        "flex cursor-default items-center justify-center py-1",
        className,
      )}
      {...props}
    >
      <ChevronDownIcon className="size-4" />
    </SelectPrimitive.ScrollDownButton>
  );
}

// Radix refuses an empty item value, so a "no choice" option travels under this one.
const NONE = "__none__";

/** The whole select for a flat list of options, the empty string being a legal "none". */
function SelectField({
  value,
  onValueChange,
  options,
  label,
  id,
  disabled,
  size,
  className,
}: {
  value: string;
  onValueChange: (v: string) => void;
  options: { value: string; label: React.ReactNode }[];
  /** The accessible name, where no <label> wraps it. */
  label?: string;
  id?: string;
  disabled?: boolean;
  size?: "sm" | "default";
  className?: string;
}) {
  const wire = (v: string) => (v === "" ? NONE : v);
  return (
    <Select
      value={wire(value)}
      disabled={disabled}
      onValueChange={(v) => onValueChange(v === NONE ? "" : v)}
    >
      <SelectTrigger
        id={id}
        size={size}
        aria-label={label}
        className={cn("min-w-0", className)}
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent position="popper">
        {options.map((o) => (
          <SelectItem key={o.value} value={wire(o.value)}>
            {o.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}

export {
  Select,
  SelectContent,
  SelectField,
  SelectGroup,
  SelectItem,
  SelectLabel,
  SelectScrollDownButton,
  SelectScrollUpButton,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
};
