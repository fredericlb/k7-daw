// LaneWarning.tsx — the gutter's warning icon on a lane the engine reported a non-fatal problem for
// (the track or one of its modules). Says the sentences on hover and opens the problems list on click.

import { TriangleAlert } from "@studio/ui/icons";

export function LaneWarning({
  sentences,
  onOpen,
}: {
  sentences: string[];
  onOpen?: () => void;
}) {
  const title = sentences.join("\n");
  return (
    <button
      type="button"
      className="ml-1 inline-flex shrink-0 cursor-pointer text-warn"
      title={title}
      aria-label={title}
      onPointerDown={(e) => e.stopPropagation()}
      onClick={(e) => {
        e.stopPropagation();
        onOpen?.();
      }}
    >
      <TriangleAlert className="h-3 w-3" />
    </button>
  );
}
