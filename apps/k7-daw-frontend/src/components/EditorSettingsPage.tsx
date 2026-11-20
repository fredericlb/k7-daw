// EditorSettingsPage.tsx — the editor's settings, a TAB of the settings panel: vim mode, a switch in a row.
// The STATE lives in App (it also drives the editor toolbar's Vim button), so this is a second control for it.

import { Row, RowButton, RowGroup } from "@studio/ui/row-group";
import { Switch } from "@studio/ui/switch";
import { m } from "../paraglide/messages.js";

export function EditorSettingsPage({
  vimMode,
  onToggleVim,
  onOpenCommands,
}: {
  vimMode: boolean;
  onToggleVim: () => void;
  /** Opens the command editor, the one settings screen that is a full page. */
  onOpenCommands: () => void;
}) {
  return (
    <>
      <RowGroup>
        <RowButton
          disclosure
          detail={m["editorSettings.commandsHint"]()}
          onClick={onOpenCommands}
        >
          {m["editorSettings.commands"]()}
        </RowButton>
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
      <p className="type-status px-3.5 pt-2 text-fg-3">
        {m["editorSettings.note"]()}
      </p>
    </>
  );
}
