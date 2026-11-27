// ExternalFooter.tsx — the foot of a surface alone: one command line, then the status and full screen. The line is
// not a command BAR (no completion, no palette): what makes those worth having is a keyboard. See ../../docs/external.md.

import { Maximize, Minimize, Terminal } from "@studio/ui/icons";
import type { ReactNode, RefObject } from "react";
import type { useFullscreen } from "../hooks/useFullscreen";
import { m } from "../paraglide/messages.js";

export function ExternalFooter({
  line,
  onLine,
  onRun,
  lineRef,
  status,
  fullscreen,
  back,
  className,
}: {
  line: string;
  onLine: (v: string) => void;
  /** A submitted line, already trimmed and not empty. */
  onRun: (line: string) => void;
  lineRef: RefObject<HTMLInputElement | null>;
  status: string;
  fullscreen: ReturnType<typeof useFullscreen>;
  /** The way back to the surfaces list, when this screen draws one in its foot. */
  back: ReactNode;
  className?: string;
}) {
  return (
    <div className={className}>
      <form
        className="flex shrink-0 items-center gap-2 border-t border-hairline px-3 py-1"
        onSubmit={(e) => {
          e.preventDefault();
          const l = line.trim();
          if (!l) return;
          onLine("");
          onRun(l);
        }}
      >
        <Terminal className="h-4 w-4 shrink-0 text-fg-2" />
        <input
          ref={lineRef}
          value={line}
          onChange={(e) => onLine(e.target.value)}
          placeholder={m["extsrf.commandPlaceholder"]()}
          spellCheck={false}
          autoCapitalize="off"
          autoCorrect="off"
          className="min-w-0 flex-1 bg-transparent py-0.5 rounded-sm font-mono text-xs outline-none focus-visible:ring-1 focus-visible:ring-focus placeholder:text-fg-2"
        />
      </form>
      <footer className="flex shrink-0 items-center gap-2 border-t border-hairline px-3 py-1">
        {back}
        {status ? (
          <p className="min-w-0 flex-1 truncate text-xs text-fg-2">
            {status}
          </p>
        ) : null}
        {/* The other half of "an app" on a device that could not install one — ../docs/external.md. */}
        {fullscreen.available ? (
          <button
            type="button"
            onClick={fullscreen.toggle}
            title={
              fullscreen.active
                ? m["extsrf.exitFullscreen"]()
                : m["extsrf.fullscreen"]()
            }
            className="ml-auto flex shrink-0 items-center rounded px-1 py-0.5 text-fg-2 hover:bg-row"
          >
            {fullscreen.active ? (
              <Minimize className="h-4 w-4" />
            ) : (
              <Maximize className="h-4 w-4" />
            )}
          </button>
        ) : null}
      </footer>
    </div>
  );
}
