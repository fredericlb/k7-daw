// AutomationHeader.tsx — an automation lane's header in the timeline's gutter: its param's name, and on a click a
// bubble with "Remove this automation", which runs `track automation remove <track> <param>`. The binding goes;
// the curve file stays, since another knob may read it (verbs' automation.rs, docs/timeline.md).

import { Trash2 } from "@studio/ui/icons";
import { Popover, PopoverAnchor, PopoverContent } from "@studio/ui/popover";
import { useState } from "react";
import { m } from "../paraglide/messages.js";

export function AutomationHeader({
  trackId,
  module,
  param,
  curve,
  readOnly,
  width,
  className,
  gutter,
  run,
}: {
  trackId: string;
  module: string;
  param: string;
  curve: string;
  readOnly?: boolean;
  width: number;
  className: string;
  /** The gutter's own press, click and double-click: the row is still selected and dragged as any other. */
  gutter: {
    onPointerDown: (e: React.PointerEvent) => void;
    onClick: (e: React.MouseEvent) => void;
    onDoubleClick: () => void;
  };
  run: (line: string) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverAnchor asChild>
        <button
          type="button"
          aria-haspopup={readOnly ? undefined : "dialog"}
          style={{ width }}
          onPointerDown={gutter.onPointerDown}
          onDoubleClick={gutter.onDoubleClick}
          onClick={(e) => {
            gutter.onClick(e);
            if (!readOnly && !e.shiftKey) setOpen(true);
          }}
          className={`text-left ${className}`}
          title={`${module} · ${param} ← ${curve}`}
        >
          <span className="truncate">{param}</span>
        </button>
      </PopoverAnchor>
      <PopoverContent side="right" align="start" className="w-auto p-1">
        <button
          type="button"
          onClick={() => {
            setOpen(false);
            run(`track automation remove ${trackId} ${param}`);
          }}
          className="type-body flex min-h-8 w-full cursor-pointer items-center gap-2 rounded-xl px-3 text-left text-error outline-none hover:bg-on-fill focus-visible:bg-on-fill"
        >
          <Trash2 className="size-4" />
          {m["tl.autoRemove"]()}
        </button>
      </PopoverContent>
    </Popover>
  );
}
