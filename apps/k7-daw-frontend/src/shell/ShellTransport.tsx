// ShellTransport.tsx — the bar's buttons: play/pause, stop, loop and record, each dropping focus on click so the
// keys stay the shell's. A set draws only the first two. Record's long press (or right click, or ArrowDown) offers
// the output recording, which the arrangement's record does not cover.
import type { EngineClient } from "@studio/engine-browser";
import type { TransportAction } from "@studio/engine-protocol";
import { CapsuleButton } from "@studio/ui/capsule";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Circle, Pause, Play, Repeat, Square } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import {
  type KeyboardEvent as ReactKeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
  useRef,
  useState,
} from "react";
import type { SetStatus } from "../hooks/useLogs";
import { blurOnClick } from "../keys";
import { midiUnavailableText } from "../midiMessage";
import { m } from "../paraglide/messages.js";

/** How long a press on Record is held before it means "record the output". */
const LONG_PRESS_MS = 500;

export interface OutputRecord {
  recording: boolean;
  filing: boolean;
  ready: boolean;
  onToggle: () => void;
}

export function ShellTransport({
  playing,
  transportWaiting,
  recordWithoutArm,
  transport,
  noArrangement,
  recordEnabled,
  midiArmed,
  openMidiInputs,
  setStatus,
  engineRef,
  armForRecord,
  loopOn = false,
  onLoop,
  output,
}: {
  playing: boolean;
  /** The engine cannot answer the transport yet — loading, or the project still on its way. */
  transportWaiting: boolean;
  recordWithoutArm: boolean;
  transport: (a: TransportAction) => void;
  noArrangement: boolean;
  recordEnabled: boolean;
  midiArmed: string | null;
  openMidiInputs: () => Promise<boolean>;
  setStatus: SetStatus;
  engineRef: RefObject<EngineClient | undefined>;
  /** Arm a track when Record is turned on with none armed; false when there was none to arm. */
  armForRecord?: () => boolean;
  loopOn?: boolean;
  onLoop?: () => void;
  /** The output recorder, when this project has one. */
  output?: OutputRecord;
}) {
  const [menu, setMenu] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const longPressed = useRef(false);
  const outLabel = output?.recording ? m["record.stop"]() : m["record.start"]();
  const startPress = (e: ReactPointerEvent) => {
    // Radix would open the menu on pointerdown: the press decides instead.
    e.preventDefault();
    longPressed.current = false;
    if (!output) return;
    timer.current = setTimeout(() => {
      longPressed.current = true;
      setMenu(true);
    }, LONG_PRESS_MS);
  };
  const endPress = () => {
    if (timer.current) clearTimeout(timer.current);
    timer.current = null;
  };
  const toggleRecord = () => {
    // A MIDI-armed lane is recorded by THIS record too: the press is the gesture Web MIDI's request rides.
    if (!recordEnabled && midiArmed)
      void openMidiInputs().then((ok) => {
        if (!ok) setStatus(midiUnavailableText());
      });
    // Turning record ON with nothing armed arms a track first, so Play does not go dead under it.
    if (!recordEnabled) armForRecord?.();
    engineRef.current?.setRecordEnabled(!recordEnabled);
  };
  return (
    <>
      <CapsuleButton
        pressed={playing}
        // Dead while the engine loads a project, and in record mode with nothing armed — a rolling transport can always stop.
        disabled={transportWaiting || (!playing && recordWithoutArm)}
        onClick={(e) => {
          transport(playing ? "stop" : "play");
          blurOnClick(e);
        }}
        title={
          transportWaiting
            ? m["transport.loading"]()
            : !playing && recordWithoutArm
              ? m["transport.noArm"]()
              : playing
                ? m["transport.pause"]()
                : m["transport.play"]()
        }
        aria-label={
          transportWaiting
            ? m["transport.loadingLabel"]()
            : playing
              ? m["transport.pause"]()
              : m["transport.play"]()
        }
        aria-busy={transportWaiting}
        className="bg-primary text-primary-foreground shadow-(--k7-highlight) aria-pressed:bg-primary aria-pressed:text-primary-foreground hover:bg-primary-hover"
      >
        {transportWaiting ? (
          <Spinner className="text-primary-foreground" />
        ) : playing ? (
          <Pause />
        ) : (
          <Play />
        )}
      </CapsuleButton>
      <CapsuleButton
        onClick={(e) => {
          transport("stop");
          // Zero on a set too: every record is on a platter, and the timeline holds only the click.
          transport("rewind");
          blurOnClick(e);
        }}
        aria-label={m["transport.stop"]()}
        title={m["transport.stop"]()}
      >
        <Square />
      </CapsuleButton>
      {/* Loop and record are the ARRANGEMENT's half: a set has no bars to count and no lane to arm. */}
      {!noArrangement && onLoop && (
        <CapsuleButton
          pressed={loopOn}
          aria-label={m["transport.loop"]()}
          onClick={(e) => {
            onLoop();
            blurOnClick(e);
          }}
          title={loopOn ? m["transport.loopOn"]() : m["transport.loopOff"]()}
        >
          <Repeat />
        </CapsuleButton>
      )}
      {!noArrangement && (
        <DropdownMenu open={menu} onOpenChange={setMenu}>
          <DropdownMenuTrigger asChild>
            <CapsuleButton
              pressed={recordEnabled}
              // Not turned ON while the project loads: the load's silent dry run would roll it and its stop switch it off.
              disabled={transportWaiting && !recordEnabled}
              onPointerDown={startPress}
              onPointerUp={endPress}
              onPointerLeave={endPress}
              onClick={(e) => {
                blurOnClick(e);
                if (longPressed.current) return;
                toggleRecord();
              }}
              onKeyDown={(e: ReactKeyboardEvent) => {
                // Enter and Space are the press; ArrowDown stays the trigger's own way into the menu.
                if (e.key !== "Enter" && e.key !== " ") return;
                e.preventDefault();
                if (!e.repeat) toggleRecord();
              }}
              onContextMenu={(e) => {
                if (!output) return;
                e.preventDefault();
                setMenu(true);
              }}
              title={m["transport.record"]()}
              aria-label={m["transport.recordLabel"]()}
            >
              <Circle
                className={recordEnabled ? "fill-record text-record" : ""}
              />
            </CapsuleButton>
          </DropdownMenuTrigger>
          {output && (
            <DropdownMenuContent align="start" side="top" className="w-[260px]">
              <DropdownMenuItem
                disabled={output.filing || !output.ready}
                onSelect={output.onToggle}
              >
                <Circle
                  className={
                    output.recording ? "fill-record text-record" : undefined
                  }
                />
                {outLabel}
              </DropdownMenuItem>
            </DropdownMenuContent>
          )}
        </DropdownMenu>
      )}
    </>
  );
}
