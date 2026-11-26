// PanelRail.tsx — the buttons that open a right panel, as one vertical toolbar on the screen's right edge, Settings last.
// It is folded by default under a chevron at the top right; a phone has no rail, its overflow menu holds the same buttons (docs/shell.md).

import { Capsule, CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import { ChevronDown, ChevronUp, Settings } from "@studio/ui/icons";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import type { HeaderPanel } from "./ProjectHeader";
import { PANEL_DEFS } from "./panels";

// A rail button is a round square, not the header's wide segment.
const SQUARE =
  "size-10 min-w-0 rounded-full px-0 desk:size-8 desk:min-w-0 desk:rounded-full";

export function PanelRail({
  panels,
  settingsPressed,
  onSettings,
}: {
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Capsule
      aria-label={m["shell.panels"]()}
      aria-orientation="vertical"
      className="absolute top-[calc(var(--k7-header-h)+var(--k7-panel-gap-top))] right-3 z-(--k7-z-header) h-auto desk:h-auto flex-col gap-1.5 p-1"
    >
      <CapsuleButton
        className={SQUARE}
        aria-expanded={open}
        aria-label={open ? m["shell.panelsHide"]() : m["shell.panelsShow"]()}
        title={open ? m["shell.panelsHide"]() : m["shell.panelsShow"]()}
        onClick={() => setOpen((o) => !o)}
      >
        {open ? <ChevronUp /> : <ChevronDown />}
      </CapsuleButton>
      {open && (
        <RailButtons
          panels={panels}
          settingsPressed={settingsPressed}
          onSettings={onSettings}
        />
      )}
    </Capsule>
  );
}

// The rail's content once unfolded: the panels' buttons, then Settings.
function RailButtons({
  panels,
  settingsPressed,
  onSettings,
}: {
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
}) {
  return (
    <>
      <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
      {panels.map((p) => {
        const def = PANEL_DEFS[p.id];
        return (
          <CapsuleButton
            key={p.id}
            className={SQUARE}
            pressed={p.pressed}
            disabled={p.disabled}
            disabledReason={p.title}
            aria-label={def.title()}
            title={p.title ?? def.title()}
            onClick={p.onPress}
          >
            <def.icon />
          </CapsuleButton>
        );
      })}
      <CapsuleDivider className="mx-0 my-0.5 h-px w-5" />
      <CapsuleButton
        className={SQUARE}
        pressed={settingsPressed}
        aria-label={m["settings.menu"]()}
        title={m["settings.menu"]()}
        onClick={onSettings}
      >
        <Settings />
      </CapsuleButton>
    </>
  );
}
