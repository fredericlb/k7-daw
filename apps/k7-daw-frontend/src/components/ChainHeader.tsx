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
  { kind: "audio", label: () => m["chain.audioIn"](), Icon: Mic },
  { kind: "midi", label: () => m["chain.midiIn"](), Icon: KeyboardMusic },
] as const;

// A taken input stays pressable (aria-disabled) so a press can say why it does nothing.
const DISABLED = "aria-disabled:cursor-not-allowed aria-disabled:opacity-40";

export function ChainHeader({
  name,
  moduleCount,
  armed,
  onArm,
  controls,
  onControls,
  hasInput,
  onInput,
  onReason,
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
  /** Where the reason a taken input chip does nothing goes when it is pressed (the status line). */
  onReason?: (message: string) => void;
  showCurves: boolean;
  onToggleCurves?: () => void;
}) {
  const cards = moduleCount !== undefined;
  return (
    <DockHeaderRow>
      <div className="flex min-w-0 flex-1 items-center gap-2">
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
            INPUTS.map(({ kind, label, Icon }) => {
              const text = label();
              return (
                <button
                  key={kind}
                  type="button"
                  onClick={() =>
                    hasInput
                      ? onReason?.(m["chain.inputTaken"]())
                      : onInput(kind)
                  }
                  aria-disabled={hasInput || undefined}
                  aria-label={text}
                  title={
                    hasInput
                      ? `${text} — ${m["chain.inputTaken"]()}`
                      : m["chain.inputHint"]({ label: text })
                  }
                  className={`${CHIP} ${DISABLED}`}
                >
                  <Icon className="size-4" />
                </button>
              );
            })}
          {onToggleCurves && (
            <button
              type="button"
              onClick={onToggleCurves}
              aria-pressed={showCurves}
              aria-label={m["chain.automation"]()}
              title={
                showCurves ? m["chain.curvesHide"]() : m["chain.curvesShow"]()
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
