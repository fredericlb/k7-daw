// PanelRail.tsx — the buttons that open a right panel, as one vertical toolbar on the screen's right edge, Settings last.
// A phone has no rail: its overflow menu holds the same buttons. The right panel and the dock keep clear of it (docs/shell.md).

import { Capsule, CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import { Settings } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import type { HeaderPanel } from "./ProjectHeader";
import { PANEL_DEFS } from "./panels";

export function PanelRail({
  panels,
  settingsPressed,
  onSettings,
}: {
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
}) {
  return (
    <Capsule
      aria-label={m["shell.panels"]()}
      aria-orientation="vertical"
      className="absolute top-[calc(var(--k7-header-h)+var(--k7-panel-gap-top))] right-3 z-(--k7-z-header) h-auto flex-col"
    >
      {panels.map((p) => {
        const def = PANEL_DEFS[p.id];
        return (
          <CapsuleButton
            key={p.id}
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
        pressed={settingsPressed}
        aria-label={m["settings.menu"]()}
        title={m["settings.menu"]()}
        onClick={onSettings}
      >
        <Settings />
      </CapsuleButton>
    </Capsule>
  );
}
