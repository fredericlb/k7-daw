// DockFitButton.tsx — the clip editors' first title-bar control: it centres the timeline on the clip being edited (`ui fit`).
// What a reader gets wrong: it moves the TIMELINE, not the editor; the editor keeps its own window (docs/editors.md).

import { DockHeaderRow } from "@studio/ui/editor-dock";
import { Focus } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";

export function DockFitButton({
  command,
}: {
  command?: (line: string) => unknown;
}) {
  if (!command) return null;
  return (
    <DockHeaderRow>
      <button
        type="button"
        aria-label={m["dock.fit"]()}
        title={m["dock.fit"]()}
        onClick={() => void command("ui fit")}
        className="glass grid size-9 shrink-0 cursor-pointer place-items-center rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring)"
      >
        <Focus className="size-4" />
      </button>
    </DockHeaderRow>
  );
}
