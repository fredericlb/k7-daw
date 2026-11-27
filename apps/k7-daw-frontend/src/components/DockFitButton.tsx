// DockFitButton.tsx — the clip editors' first title-bar control: it centres the timeline on the clip being edited (`ui fit`).
// What a reader gets wrong: it moves the TIMELINE, not the editor; the editor keeps its own window (docs/editors.md).

import { Button } from "@studio/ui/button";
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
      <Button
        variant="glass"
        size="icon-md"
        className="shrink-0"
        aria-label={m["dock.fit"]()}
        title={m["dock.fit"]()}
        onClick={() => void command("ui fit")}
      >
        <Focus className="size-4" />
      </Button>
    </DockHeaderRow>
  );
}
