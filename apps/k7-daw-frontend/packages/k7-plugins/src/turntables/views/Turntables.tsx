// Turntables.tsx — the desk AS A SURFACE: what a host mounts for `type: turntables`. It checks
// the mapping first and draws the missing wiring instead of a desk whose knobs go nowhere.
// See packages/song/docs/surfaces.md and ../../docs/hosting.md.

import type { MixView as Mapping } from "@studio/song-schema/yaml/mixViewSchema";
import { Unplug } from "@studio/ui/icons";
import { CodeText } from "../../CodeText";
import { m } from "../../paraglide/messages.js";
import type { Deck } from "../engine";
import { unmappedControls } from "../mapping";
import type { DeckSet } from "../useDeckSet";
import { MixView } from "./MixView";

/** The desk, or the reason it cannot be drawn — a GATE, since these components make a desk a desk. */
export function Turntables({
  deck,
  set,
  mapping,
  /** Which playlist the surface names — carried through, `useDeckSet` has already read the file. */
  playlist,
  /** The surface's own file, so the message can name what to edit. */
  file,
}: {
  deck: Deck;
  set: DeckSet;
  mapping: Mapping | undefined;
  playlist?: string;
  file: string;
}) {
  const missing = unmappedControls(mapping);
  if (missing.length)
    return (
      <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-4 p-8">
        <Unplug className="h-8 w-8 text-muted-foreground" aria-hidden="true" />
        <ul className="flex max-w-xl flex-col gap-1.5">
          {missing.map((c) => (
            <li
              key={c.id}
              className="rounded border border-destructive/40 bg-destructive/10 px-3 py-2 text-sm text-destructive"
            >
              {m["gate.unmapped"]({ id: c.id })}
            </li>
          ))}
        </ul>
        {/* The fix, not just the fault: a line somebody can copy, in the file it goes in. */}
        <p className="max-w-xl text-center text-xs text-muted-foreground">
          <CodeText
            text={m["gate.fix"]({ file, shape: "{ module: …, param: … }" })}
          />
        </p>
      </div>
    );

  return <MixView deck={deck} set={set} playlist={playlist} file={file} />;
}
