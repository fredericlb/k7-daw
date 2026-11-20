// EditorSettingsPage.tsx — the editor's settings, a TAB of the settings panel: vim mode, a switch in a row.
// The STATE lives in App (it also drives the editor toolbar's Vim button), so this is a second control for it.

import { Row, RowGroup } from "@studio/ui/row-group";
import { Switch } from "@studio/ui/switch";
import { m } from "../paraglide/messages.js";

export function EditorSettingsPage({
  vimMode,
  onToggleVim,
}: {
  vimMode: boolean;
  onToggleVim: () => void;
}) {
  return (
    <RowGroup>
      <Row
        detail={m["editorSettings.vimHint"]()}
        trailing={
          <Switch
            checked={vimMode}
            onCheckedChange={onToggleVim}
            aria-label={m["editorSettings.vim"]()}
          />
        }
      >
        {m["editorSettings.vim"]()}
      </Row>
    </RowGroup>
  );
}
