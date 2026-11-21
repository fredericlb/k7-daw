// leaveAsk.ts — the question asked before leaving a project with unsaved work, drawn by the app: a webview may
// implement no `window.confirm` at all (WKWebView in the desktop app answers false without showing anything).

import { askConfirm } from "@studio/project-shell/ask";
import { hasUnsavedWork, unsavedKind } from "@studio/ui-editor/unsaved";
import { m } from "./paraglide/messages.js";

/** True when leaving is fine: nothing is unsaved, or the person said yes. */
export const mayLeave = (): Promise<boolean> =>
  hasUnsavedWork()
    ? askConfirm({
        title:
          unsavedKind() === "recording"
            ? m["leave.recording"]()
            : m["leave.unsaved"](),
        confirmLabel: m["leave.confirm"](),
        cancelLabel: m["common.cancel"](),
        danger: true,
      })
    : Promise.resolve(true);
