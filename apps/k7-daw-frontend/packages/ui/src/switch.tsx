"use client";

import { Switch as SwitchPrimitive } from "radix-ui";
import type * as React from "react";

import { cn } from "./lib/utils";

// A 51 x 31 track with a 27 px knob; the hit area is stretched to 44 px by a pseudo-element.
function Switch({
  className,
  size = "default",
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root> & {
  size?: "sm" | "default";
}) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      data-size={size}
      className={cn(
        "peer group/switch relative inline-flex shrink-0 cursor-pointer items-center rounded-2xl p-0.5 transition-colors outline-none before:absolute before:inset-x-0 before:-inset-y-[7px] before:content-[''] focus-visible:shadow-(--k7-focus-ring) disabled:cursor-not-allowed disabled:opacity-40 data-[size=default]:h-[31px] data-[size=default]:w-[51px] data-[size=sm]:h-5 data-[size=sm]:w-8 data-[size=sm]:rounded-[10px] data-[state=checked]:bg-primary data-[state=unchecked]:bg-(--k7-switch-off)",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        className="pointer-events-none block rounded-full bg-white shadow-(--k7-shadow-knob) transition-transform duration-200 group-data-[size=default]/switch:size-[27px] group-data-[size=sm]/switch:size-4 group-data-[size=default]/switch:data-[state=checked]:translate-x-5 group-data-[size=sm]/switch:data-[state=checked]:translate-x-3 data-[state=unchecked]:translate-x-0"
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
