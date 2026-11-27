// PanelButton.tsx — the capsule button of one panel: its glyph, name and pressed state, from the panel's own record.
// The rail and the dock's capsule draw the same panels; a count on the corner is the one thing a caller adds.

import { CapsuleButton } from "@studio/ui/capsule";
import type { ReactNode } from "react";
import type { HeaderPanel } from "./ProjectHeader";
import { PANEL_DEFS } from "./panels";
import { WarnBadge } from "./WarnBadge";

export function PanelButton({
  panel,
  size,
  children,
}: {
  panel: HeaderPanel;
  size: "float" | "icon";
  /** Another corner mark, beside the problem count. */
  children?: ReactNode;
}) {
  const def = PANEL_DEFS[panel.id];
  return (
    <CapsuleButton
      size={size}
      className="relative"
      pressed={panel.pressed}
      disabled={panel.disabled}
      disabledReason={panel.title}
      aria-label={def.title()}
      title={panel.title ?? def.title()}
      onClick={panel.onPress}
    >
      <def.icon />
      <WarnBadge n={panel.badge ?? 0} />
      {children}
    </CapsuleButton>
  );
}
