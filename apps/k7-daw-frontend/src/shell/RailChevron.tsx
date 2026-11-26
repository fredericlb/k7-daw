// RailChevron.tsx — the chevron that unfolds the right-edge rail upward, carrying the unsaved and problem counts while folded.
// A desk draws it last of its floating controls (ProjectHeader.tsx), a phone at its floating bar's right end.
import { Badge } from "@studio/ui/badge";
import { Capsule, CapsuleButton } from "@studio/ui/capsule";
import { ChevronDown, ChevronUp } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { WarnBadge } from "./WarnBadge";

export function RailChevron({
  open,
  onToggle,
  unsaved,
  problems,
  className,
}: {
  open: boolean;
  onToggle: () => void;
  unsaved: number;
  problems: number;
  className?: string;
}) {
  const label = open ? m["shell.panelsHide"]() : m["shell.panelsShow"]();
  return (
    <Capsule className={className ?? "float"}>
      <CapsuleButton
        className="relative"
        aria-expanded={open}
        aria-label={label}
        title={label}
        onClick={onToggle}
      >
        {open ? <ChevronDown /> : <ChevronUp />}
        {!open && <WarnBadge n={problems} />}
        {!open && unsaved > 0 && (
          <Badge
            variant="count-sm"
            aria-hidden
            className="pointer-events-none absolute -top-1.5 -left-1.5"
          >
            {unsaved}
          </Badge>
        )}
      </CapsuleButton>
    </Capsule>
  );
}
