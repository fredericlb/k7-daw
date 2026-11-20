// a11y.ts — what an editor tells assistive technology: its accessible name, and the vim mode it is in.
// The one thing a reader would get wrong: the mode is read from vim's own "vim-mode-change" event, so
// this must sit AFTER vim() in the extension list — before it, there is no vim yet to listen to.
// See apps/k7-daw-frontend/packages/ui-editor/docs/a11y.md.

import type { Extension } from "@codemirror/state";
import { EditorView, type Panel, showPanel } from "@codemirror/view";
import { getCM } from "@replit/codemirror-vim";

/** The textbox's accessible name — the file it shows, e.g. `project.k7x — editor`. */
export const editorLabel = (label: string): Extension =>
  EditorView.contentAttributes.of({ "aria-label": label });

/** What vim's mode-change event carries. */
interface ModeChange {
  mode: string;
  subMode?: string;
}

/** The mode as vim's own status line spells it: NORMAL, INSERT, VISUAL, VISUAL LINE, VISUAL BLOCK, REPLACE. */
export function vimModeText(e: ModeChange): string {
  const sub =
    e.subMode === "linewise"
      ? " line"
      : e.subMode === "blockwise"
        ? " block"
        : "";
  return `${e.mode}${sub}`.toUpperCase();
}

/** The status line under the text: the cursor's line:col in mono and, with vim on, the mode as a badge (`role="status"` named `vim`). */
export const vimModeStatus = (): Extension => [
  EditorView.baseTheme({
    ".cm-status": {
      display: "flex",
      alignItems: "center",
      gap: "10px",
      padding: "4px 12px",
      fontFamily: "var(--k7-font-mono, monospace)",
      fontSize: "12px",
    },
    ".cm-vim-mode": {
      padding: "2px 10px",
      borderRadius: "999px",
      fontWeight: "700",
      fontSize: "11px",
      background: "var(--k7-on-fill, rgba(255,255,255,.17))",
      color: "var(--k7-on, #d6c8ff)",
    },
    '.cm-vim-mode[data-vim-mode="insert"]': {
      background: "var(--k7-on, #d6c8ff)",
      color: "var(--k7-ink, #2a1640)",
    },
  }),
  showPanel.of((view): Panel => {
    const dom = document.createElement("div");
    dom.className = "cm-status";
    const badge = document.createElement("span");
    badge.className = "cm-vim-mode";
    badge.setAttribute("role", "status");
    badge.setAttribute("aria-label", "vim");
    const pos = document.createElement("span");
    pos.className = "cm-cursor-pos";
    dom.append(badge, pos);
    const show = (e: ModeChange) => {
      badge.textContent = vimModeText(e);
      badge.dataset.vimMode = e.mode;
    };
    show({ mode: "normal" });
    const place = (v: EditorView) => {
      const head = v.state.selection.main.head;
      const line = v.state.doc.lineAt(head);
      pos.textContent = `${line.number}:${head - line.from + 1}`;
    };
    place(view);
    const listen = (): boolean => {
      const cm = getCM(view);
      cm?.on("vim-mode-change", show);
      return !!cm;
    };
    // Vim's plugin may be built after the panel: one retry once the view is up.
    return {
      dom,
      mount: () => void (listen() || queueMicrotask(listen)),
      update: (u) => {
        if (u.selectionSet || u.docChanged) place(u.view);
      },
    };
  }),
];
