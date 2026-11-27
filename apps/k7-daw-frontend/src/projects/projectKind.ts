// projectKind.ts — what a home-page row says about a project beyond its name: the archetype its
// declared surfaces amount to, and how long ago it last changed. Pure, so both are tested without
// a page. The archetype rule is Rust's (song/src/reads/misc.rs). See ../../docs/projects.md.

import { shapeOfSurfaces as shapeOf } from "@studio/k7-wasm/reads";
import type { ProjectShape } from "@studio/k7-wasm/types";

/** The archetype a project's surfaces read as, or null when the service is too old to say. */
export const shapeOfSurfaces = (
  surfaces: readonly string[] | undefined,
): ProjectShape | null => shapeOf(surfaces) as ProjectShape | null;

const STEPS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["minute", 60],
  ["hour", 60],
  ["day", 24],
  ["week", 7],
  ["month", 4.35],
  ["year", 12],
];

/** "3 hours ago" in `locale`, rounded to the largest unit that fits; "now" under a minute. */
export function editedAgo(at: number, now: number, locale: string): string {
  const fmt = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  let value = Math.max(0, (now - at) / 1000);
  if (value < 60) return fmt.format(0, "second");
  value /= 60;
  for (let i = 0; i < STEPS.length; i++) {
    const [unit] = STEPS[i];
    const next = STEPS[i + 1];
    if (!next || value < next[1]) return fmt.format(-Math.floor(value), unit);
    value /= next[1];
  }
  return fmt.format(-Math.floor(value), "year");
}
