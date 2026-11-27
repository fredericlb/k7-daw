// GridMenu.tsx — the builder grid's right-click menu: a short list at the pointer, nothing more.
// It is the kit's PointMenu, so it dismisses and answers the keys like every pointer menu. Rationale: docs/builder.md.

import type { LucideIcon } from "@studio/ui/icons";
import { PointMenu, PointMenuItem } from "@studio/ui/point-menu";

export interface GridMenuItem {
  label: string;
  icon: LucideIcon;
  disabled?: boolean;
  onPick: () => void;
}

export function GridMenu({
  x,
  y,
  items,
  onClose,
}: {
  /** Viewport coordinates of the right-click. */
  x: number;
  y: number;
  items: GridMenuItem[];
  onClose: () => void;
}) {
  return (
    <PointMenu x={x} y={y} onClose={onClose} className="min-w-[8rem]">
      {items.map((it) => (
        <PointMenuItem
          key={it.label}
          icon={it.icon}
          disabled={it.disabled}
          onSelect={it.onPick}
        >
          {it.label}
        </PointMenuItem>
      ))}
    </PointMenu>
  );
}
