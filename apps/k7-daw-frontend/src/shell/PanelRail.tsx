// PanelRail.tsx — the pane switcher, then the buttons that open a right panel, Settings last: one vertical toolbar on the right edge.
// It shows only once the header's chevron unfolds it; a phone has no rail, its overflow menu holds the same buttons (docs/shell.md).

import { Capsule, CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import { Settings } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import type { HeaderPane, HeaderPanel } from "./ProjectHeader";
import { PANEL_DEFS } from "./panels";

// A rail button is a round square, not the header's wide segment.
const SQUARE =
  "size-10 min-w-0 rounded-full px-0 desk:size-8 desk:min-w-0 desk:rounded-full";

export function PanelRail({
  panes,
  pane,
  onPane,
  panels,
  settingsPressed,
  onSettings,
}: {
  panes: HeaderPane[];
  pane: string;
  onPane: (id: string) => void;
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
}) {
  return (
    <Capsule
      aria-label={m["shell.panels"]()}
      aria-orientation="vertical"
      className="absolute top-[calc(var(--k7-header-h)+var(--k7-panel-gap-top))] right-3 z-(--k7-z-header) h-auto desk:h-auto flex-col gap-1.5 p-1"
    >
      <div
        role="group"
        aria-label={m["shell.pane"]()}
        className="flex flex-col gap-1.5"
      >
        {panes.map((p) => (
          <CapsuleButton
            key={p.id}
            className={SQUARE}
            pressed={pane === p.id}
            disabled={!!p.blocked}
            disabledReason={p.blocked}
            aria-label={p.label}
            title={
              p.blocked ?? (p.shortcut ? `${p.label} (${p.shortcut})` : p.label)
            }
            onClick={() => onPane(p.id)}
          >
            <p.icon />
          </CapsuleButton>
        ))}
      </div>
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
    </Capsule>
  );
}
