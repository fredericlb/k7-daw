// PlayButton.tsx — the bar's play/pause, and the host's menu behind it: a right-click, the
// keyboard's menu key or a finger held still (the app-wide long press) opens it, a plain press only plays or pauses.
// The menu is anchored on a span over the button, not on the button: a trigger would open on every press.
// See ../docs/miniPlayer.md, "The play button's menu".

import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Pause, Play } from "@studio/ui/icons";
import { type ReactNode, useState } from "react";

export function PlayButton({
  playing,
  onToggle,
  playLabel,
  pauseLabel,
  menu,
  plain = false,
}: {
  playing: boolean;
  onToggle: () => void;
  playLabel: string;
  pauseLabel: string;
  /** The menu's items; absent, the button has none and a long press does nothing. */
  menu?: ReactNode;
  /** No background of its own, for a bar whose row is a picture. */
  plain?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const button = (
    <Button
      size="icon"
      variant={plain ? "ghost" : "secondary"}
      // No text selection or iOS callout under a held finger.
      className="shrink-0 select-none [-webkit-touch-callout:none]"
      onClick={onToggle}
      aria-label={playing ? pauseLabel : playLabel}
      aria-haspopup={menu ? "menu" : undefined}
      onContextMenu={
        menu
          ? (e) => {
              e.preventDefault();
              setOpen(true);
            }
          : undefined
      }
    >
      {playing ? <Pause className="h-4 w-4" /> : <Play className="h-4 w-4" />}
    </Button>
  );
  if (!menu) return button;

  return (
    <span className="relative shrink-0">
      {button}
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <span
            aria-hidden="true"
            tabIndex={-1}
            className="pointer-events-none absolute inset-0"
          />
        </DropdownMenuTrigger>
        <DropdownMenuContent side="top" align="start" className="w-64">
          {menu}
        </DropdownMenuContent>
      </DropdownMenu>
    </span>
  );
}
