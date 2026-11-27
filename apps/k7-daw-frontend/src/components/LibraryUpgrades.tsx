// LibraryUpgrades.tsx — above the file tree: a library played at another version than its pin, a
// library this workspace could move to a newer version of, and one whose pinned version needs a newer
// ENGINE. Both buttons run `library upgrade`, the palette's verb, so the pin is written one way only.
// See ../../../../docs/library-versions-plan.md.

import { Button } from "@studio/ui/button";
import { TriangleAlert } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import type { ModuleLibrary } from "@studio/web-protocol";
import { useState } from "react";
import { m } from "../paraglide/messages.js";

/** A library this machine lacks the pinned version of, and the version it is played at instead. */
export interface PinSubstitution {
  library: string;
  pinned: number;
  used: number;
}

/** The libraries played at another version than the workspace pins, as the listing says. */
export function pinSubstitutions(
  levels: readonly ModuleLibrary[],
): PinSubstitution[] {
  return levels.flatMap((l) => {
    const v = l.version;
    return v?.used !== undefined && typeof v.pinned === "number"
      ? [{ library: l.ns, pinned: v.pinned, used: v.used }]
      : [];
  });
}

/** One substituted library's line, and the button that pins the version it is played at. */
export function SubstitutionLine({
  s,
  busy,
  onPin,
}: {
  s: PinSubstitution;
  busy: string | null;
  onPin: (s: PinSubstitution) => void;
}) {
  return (
    <div className="flex items-center gap-2 text-amber-500">
      <TriangleAlert className="h-3.5 w-3.5 shrink-0" />
      <span className="flex-1">
        {m["libs.pinMissing"]({
          name: s.library,
          pinned: s.pinned,
          used: s.used,
        })}
      </span>
      <Button
        size="sm"
        variant="outline"
        disabled={busy !== null}
        onClick={() => onPin(s)}
      >
        {busy === s.library && <Spinner size="xs" inherit className="mr-1" />}
        {m["libs.pinUsed"]({ name: s.library, used: s.used })}
      </Button>
    </div>
  );
}

export function LibraryUpgrades({
  levels,
  engineLevel,
  onUpgrade,
}: {
  levels: readonly ModuleLibrary[];
  /** The library level the engine's machine runs; undefined with no engine, as on a share. */
  engineLevel?: number;
  /** Run `library upgrade <ns> <version>`, then read the libraries again. */
  onUpgrade: (ns: string, version: number) => Promise<void>;
}) {
  const [busy, setBusy] = useState<string | null>(null);
  const run = (ns: string, version: number) => {
    setBusy(ns);
    void onUpgrade(ns, version).finally(() => setBusy(null));
  };
  const substituted = pinSubstitutions(levels);
  const ready = levels.filter(
    (l) => l.version?.upgrade !== undefined && l.version.used === undefined,
  );
  const tooNew =
    engineLevel === undefined
      ? []
      : levels.filter((l) => (l.version?.requires ?? 1) > engineLevel);
  if (!substituted.length && !ready.length && !tooNew.length) return null;
  return (
    <div className="flex flex-col gap-1 border-b border-hairline px-2 py-1.5 text-xs">
      {substituted.map((s) => (
        <SubstitutionLine
          key={`pin-${s.library}`}
          s={s}
          busy={busy}
          onPin={(p) => run(p.library, p.used)}
        />
      ))}
      {tooNew.map((l) => (
        <div
          key={`engine-${l.ns}`}
          className="flex items-center gap-2 text-amber-500"
        >
          <TriangleAlert className="h-3.5 w-3.5 shrink-0" />
          <span className="flex-1">
            {m["libs.engineTooOld"]({
              name: l.ns,
              version: String(l.version?.used ?? l.version?.pinned),
              need: l.version?.requires ?? 1,
              level: engineLevel ?? 1,
            })}
          </span>
        </div>
      ))}
      {ready.map((l) => {
        const to = l.version?.upgrade as number;
        return (
          <div key={l.ns} className="flex items-center gap-2">
            <span className="flex-1 truncate text-fg-2">
              {m["libs.upgradeReady"]({
                name: l.ns,
                from: String(l.version?.pinned),
                to,
              })}
            </span>
            <Button
              size="sm"
              variant="outline"
              disabled={busy !== null}
              onClick={() => run(l.ns, to)}
            >
              {busy === l.ns && <Spinner size="xs" inherit className="mr-1" />}
              {m["libs.upgrade"]()}
            </Button>
          </div>
        );
      })}
    </div>
  );
}
