// surfaceMeta.tsx — a surface's type as a glyph and as a word, out of the shell's own pane table.
// An unknown type is drawn as itself, not dropped: a plugin may declare a type this build has no glyph for.

import { Disc3, type LucideIcon } from "@studio/ui/icons";
import { PANE_META, VIEWPORT_META } from "../shellMeta";

const META = { ...PANE_META, viewport: VIEWPORT_META } as Partial<
  Record<string, { icon: LucideIcon; label: string }>
>;

/** The glyph of a surface type. */
export function SurfaceIcon({ type }: { type: string }) {
  const Icon = META[type]?.icon ?? Disc3;
  return <Icon className="size-5 shrink-0" />;
}

/** The word a person reads for a surface type. */
export const typeLabel = (type: string): string => META[type]?.label ?? type;
