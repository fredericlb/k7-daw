// WarnBadge.tsx — the project's problem count on a button's corner: the header's chevron and the rail's Logs carry it.
import { Badge } from "@studio/ui/badge";

export function WarnBadge({ n }: { n: number }) {
  if (n <= 0) return null;
  return (
    <Badge
      variant="count-sm"
      aria-hidden
      className="pointer-events-none absolute -top-1.5 -right-1.5 bg-warn text-background"
    >
      {n}
    </Badge>
  );
}
