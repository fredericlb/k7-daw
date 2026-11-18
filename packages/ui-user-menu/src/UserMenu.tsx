// UserMenu.tsx — studio's user menu: @studio/ui's, with the host's settings row and its own pages.
// No app row and no sign-out row: k7 is the one studio app, and a device's session has nothing to end. See ../docs/userMenu.md.

import {
  UserMenu as BaseUserMenu,
  type UserMenuItem,
} from "@studio/ui/user-menu";
import type { StudioService } from "@studio/web-protocol/services";

export interface UserMenuProps {
  /** The app drawing the menu. */
  self: StudioService;
  /** Who is signed in, as the menu's heading; null while it is not known yet. */
  email?: string | null;
  /** Opens the host's settings page, the row under the email — the host's own navigation. */
  onSettings?: () => void;
  /** That row's name, in the host's language. */
  settingsLabel?: string;
  /** The trigger's accessible name, in the host's language. */
  label?: string;
  /** More of the host's own pages, as rows under Settings — the desktop's engine, say. */
  items?: UserMenuItem[];
}

export function UserMenu({
  email,
  onSettings,
  settingsLabel,
  label,
  items,
}: UserMenuProps) {
  return (
    <BaseUserMenu
      email={email}
      onSettings={onSettings}
      settingsLabel={settingsLabel}
      label={label}
      items={items}
    />
  );
}
