// PanelRail.tsx — git's Save and Pull, the pane switcher, the buttons that open a right panel, then Settings and Close: one vertical toolbar.
// It rises from the chevron at the bottom right once unfolded; a phone has no rail, its overflow menu holds the same buttons (docs/shell.md).

import { Capsule, CapsuleButton, CapsuleDivider } from "@studio/ui/capsule";
import { Settings, X } from "@studio/ui/icons";
import type { ReactNode } from "react";
import { m } from "../paraglide/messages.js";
import type { HeaderPane, HeaderPanel } from "./ProjectHeader";
import { PANEL_DEFS } from "./panels";
import { WarnBadge } from "./WarnBadge";

// A rail button is a round square, not the header's wide segment.
const SQUARE =
  "size-10 min-w-0 rounded-full px-0 desk:size-8 desk:min-w-0 desk:rounded-full";

// Close is a link (the projects list is another page) drawn as the rail's buttons.
const CLOSE = `inline-flex items-center justify-center text-foreground outline-none transition-colors hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring) [&_svg]:size-5 desk:[&_svg]:size-[18px] ${SQUARE}`;

export function PanelRail({
  save,
  panes,
  pane,
  onPane,
  panels,
  settingsPressed,
  onSettings,
  closeHref,
}: {
  /** Git's Save and Pull, first (HeaderGit.tsx's GitRailSave, GitRailPull), drawn with the rail's button class; null outside a git workspace. */
  save: (className: string) => ReactNode;
  panes: HeaderPane[];
  pane: string;
  onPane: (id: string) => void;
  panels: HeaderPanel[];
  settingsPressed: boolean;
  onSettings: () => void;
  /** Where Close goes: the projects list. */
  closeHref: string;
}) {
  return (
    <Capsule
      aria-label={m["shell.panels"]()}
      aria-orientation="vertical"
      className="absolute right-4 bottom-[calc(var(--k7-float-bottom)+var(--k7-float-h)+8px)] z-(--k7-z-bar) h-auto max-h-[calc(100%-var(--k7-float-bottom)-var(--k7-float-h)-24px)] flex-col gap-1.5 overflow-y-auto p-1 desk:h-auto"
    >
      {save(SQUARE)}
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
            className={`relative ${SQUARE}`}
            pressed={p.pressed}
            disabled={p.disabled}
            disabledReason={p.title}
            aria-label={def.title()}
            title={p.title ?? def.title()}
            onClick={p.onPress}
          >
            <def.icon />
            <WarnBadge n={p.badge ?? 0} />
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
      <a
        href={closeHref}
        aria-label={m["shell.close"]()}
        title={m["shell.close"]()}
        className={CLOSE}
      >
        <X />
      </a>
    </Capsule>
  );
}
