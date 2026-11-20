// ChainHeader.tsx — the chain dock's header row: the lane's name over its status, Arm, the Controls chip,
// the input buttons and the automation toggle. Expand and Close are the dock's own, not drawn here.

import { DockHeaderRow } from "@studio/ui/editor-dock";
import { KeyboardMusic, Mic, Spline } from "@studio/ui/icons";
import { TrackToggle } from "@studio/ui/track-toggle";
import { m } from "../paraglide/messages.js";

// A 36 px glass chip, the shape the kit gives a toggle inside a panel.
const CHIP =
  "glass grid h-9 min-w-9 cursor-pointer place-items-center rounded-full px-2 text-foreground focus-visible:shadow-(--k7-focus-ring) focus-visible:outline-none";

/** The two inputs a lane can be given from here, each the `track input` kind it runs. */
const INPUTS = [
  { kind: "audio", label: "audio in", Icon: Mic },
  { kind: "midi", label: "midi in", Icon: KeyboardMusic },
] as const;

/** A greyed button still shows its tooltip, which is what says why it is greyed. */
const DISABLED =
  "disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-muted-foreground";

export function ChainHeader({
  name,
  moduleCount,
  armed,
  onArm,
  controls,
  onControls,
  hasInput,
  onInput,
  showCurves,
  onToggleCurves,
}: {
  name: string;
  /** Set on the lane's own pane; absent for a popin's cards, which wear no name. */
  moduleCount?: number;
  armed?: boolean;
  onArm?: () => void;
  /** The Controls chip's pressed state and toggle; absent without cards. */
  controls?: boolean;
  onControls?: () => void;
  hasInput: boolean;
  onInput?: (kind: "audio" | "midi") => void;
  showCurves: boolean;
  onToggleCurves?: () => void;
}) {
  const cards = moduleCount !== undefined;
  return (
    <DockHeaderRow>
      <div className="flex min-w-0 flex-1 items-center gap-2">
        {cards && (
          <div className="flex min-w-0 shrink flex-col" title={name}>
            <span className="type-label truncate">{name}</span>
            <span className="type-secondary truncate text-fg-2">
              {(moduleCount === 1
                ? m["chain.status.one"]
                : m["chain.status.other"])({
                n: moduleCount,
              })}
            </span>
          </div>
        )}
        <div className="flex shrink-0 items-center gap-2">
          {onArm && (
            <TrackToggle
              kind="arm"
              on={armed}
              aria-label={m["chain.armInput"]({ name })}
              title={m["chain.armInput"]({ name })}
              onClick={onArm}
            />
          )}
          {cards && onControls && (
            <button
              type="button"
              aria-pressed={controls}
              onClick={onControls}
              className={`${CHIP} type-label aria-pressed:bg-on-fill aria-pressed:text-lavender`}
            >
              {m["chain.controls"]()}
            </button>
          )}
          {onInput &&
            INPUTS.map(({ kind, label, Icon }) => (
              <button
                key={kind}
                type="button"
                onClick={() => onInput(kind)}
                disabled={hasInput}
                aria-label={label}
                title={
                  hasInput
                    ? `${label} — this lane already has an input`
                    : `${label} — an input feeding this lane, ahead of its first node`
                }
                className={`${CHIP} ${DISABLED}`}
              >
                <Icon className="size-4" />
              </button>
            ))}
          {onToggleCurves && (
            <button
              type="button"
              onClick={onToggleCurves}
              aria-pressed={showCurves}
              aria-label={m["chain.automation"]()}
              title={
                showCurves ? "hide the curve cards" : "show the curve cards"
              }
              className={`${CHIP} aria-pressed:bg-on-fill aria-pressed:text-lavender`}
            >
              <Spline className="size-4" />
            </button>
          )}
        </div>
      </div>
    </DockHeaderRow>
  );
}
