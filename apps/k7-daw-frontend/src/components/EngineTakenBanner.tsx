// EngineTakenBanner.tsx — the strip over the open project once somebody else took its engine: who
// took it, and "take it back" (which IS opening the project again). It floats
// rather than pushing the editor down, so the gate's tree, and the project under it, never remount.
// See ../../docs/shell.md, "When somebody else takes the engine".

import { Button } from "@studio/ui/button";
import { UserRoundX } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { type Taken, takenBanner } from "./engineTaken";

export function EngineTakenBanner({
  taken,
  onTakeBack,
}: {
  taken: Taken;
  onTakeBack: () => void;
}) {
  const b = takenBanner(taken);
  const title =
    b.says === "engine"
      ? m["project.engineTakenTitle"]()
      : b.says === "refused"
        ? m["project.notYoursTitle"]()
        : m["project.takenTitle"]();
  const body =
    b.says === "engine"
      ? m["project.engineTakenBody"]({
          project: taken.tookFor ?? m["project.anotherProject"](),
        })
      : b.says === "refused"
        ? m["project.notYoursBody"]()
        : b.says === "project"
          ? m["project.takenBody"]({ heldBy: taken.heldBy })
          : m["project.takenBodyAnon"]();
  return (
    <div
      role="status"
      className="float fixed inset-x-4 top-2 z-(--k7-z-bar) mx-auto flex max-w-2xl flex-wrap items-start gap-3 rounded-(--k7-r-toast) border-(--k7-warn-toast-line) p-3.5 text-foreground"
    >
      <UserRoundX className="mt-0.5 size-[22px] shrink-0 text-warn" />
      <div className="min-w-0 flex-1 basis-60">
        <p className="type-label text-foreground">{title}</p>
        <p className="type-secondary text-fg-2">{body}</p>
      </div>
      <div className="flex shrink-0 flex-wrap items-center gap-2">
        {b.takeBack && (
          <Button size="sm" onClick={onTakeBack}>
            {b.takeBack === "engine"
              ? m["project.takeEngineBack"]()
              : m["project.takeProjectBack"]()}
          </Button>
        )}
      </div>
    </div>
  );
}
