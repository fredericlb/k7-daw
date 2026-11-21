// OverflowMenu.tsx — the phone header's one menu: pane, lane views and the panel tiles (300 wide, over a 0.35 scrim).
// Everything it holds is also a button of the Pad and Desk header: this is the same set, folded. A popover of
// plain toggle buttons, not role="menu": focus moves in on open and returns to the trigger on close.

import { METRONOME_MIN } from "@studio/engine-protocol/build";
import { Button } from "@studio/ui/button";
import { showReason } from "@studio/ui/disabled-reason";
import {
  CircleHelp,
  Ellipsis,
  type LucideIcon,
  Settings,
} from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { useModalFocus } from "@studio/ui/modal-focus";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { m } from "../paraglide/messages.js";
import { usePopin } from "../popins";
import { MetronomeIcon } from "../shellMeta";
import type { Tool } from "../timelineTools";
import { TOOL_NAME, ToolGlyph } from "./FloatingBar";
import { PANEL_DEFS, type PanelId } from "./panels";

export interface OverflowPane {
  id: string;
  label: string;
  icon: LucideIcon;
  /** Why this pane cannot be opened now; a press says it instead of opening. */
  blocked?: string;
}

export interface OverflowLaneView {
  id: string;
  label: string;
  icon: LucideIcon;
  on: boolean;
  /** A plain action (Zoom fit) rather than a switch: no checked state. */
  action?: boolean;
  onPress: () => void;
}

export interface OverflowPanel {
  id: PanelId;
  pressed: boolean;
  disabled?: boolean;
  onPress: () => void;
}

export interface OverflowTool {
  current: Tool;
  tools: readonly Tool[];
  onPick: (tool: Tool) => void;
}

function Heading({ children }: { children: ReactNode }) {
  return <p className="type-caps px-1 pt-2 pb-1 text-fg-3">{children}</p>;
}

export function OverflowMenu({
  panes,
  pane,
  onPane,
  lanes,
  lanesHeading,
  panels,
  onSettings,
  onHelp,
  tool,
  metronome,
}: {
  panes: OverflowPane[];
  pane: string;
  onPane: (id: string) => void;
  /** The current pane's views: the Lane views or the Graph views, empty where the pane has none. */
  lanes: OverflowLaneView[];
  lanesHeading: string;
  panels: OverflowPanel[];
  onSettings: () => void;
  onHelp: () => void;
  /** The timeline tool, which the phone bar does not carry; absent off the Arrangement. */
  tool?: OverflowTool;
  /** The metronome switch, which the phone bar does not carry either. */
  metronome?: {
    on: boolean;
    onPress: () => void;
    /** 0.2 … 1, with the command that commits a new level. */
    level: number;
    onLevel: (level: number) => void;
  };
}) {
  const [open, setOpen] = useState(false);
  const [dragging, setDragging] = useState<number | null>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const card = useRef<HTMLDivElement>(null);
  usePopin(open, () => setOpen(false));
  useModalFocus(card, { active: open });
  // Focus goes into the card on open, onto the pane in use, and back to the trigger on close, instead of dropping to the body.
  useEffect(() => {
    if (!open) return;
    const t = trigger.current;
    (
      card.current?.querySelector<HTMLElement>('[aria-checked="true"]') ??
      card.current?.querySelector<HTMLElement>("button")
    )?.focus();
    return () => t?.focus();
  }, [open]);
  const commitLevel = () => {
    if (dragging === null || !metronome) return;
    const v = dragging;
    setDragging(null);
    if (Math.abs(v - metronome.level) > 1e-6) metronome.onLevel(v);
  };
  const run = (fn: () => void) => () => {
    setOpen(false);
    fn();
  };
  return (
    <>
      <Button
        ref={trigger}
        variant="glass"
        size="round"
        aria-label={m["shell.overflow"]()}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="shrink-0"
      >
        <Ellipsis className="size-5" />
      </Button>
      {open && (
        <>
          <button
            type="button"
            aria-label={m["shell.overflow.close"]()}
            tabIndex={-1}
            onClick={() => setOpen(false)}
            className="scrim-menu fixed inset-0 z-(--k7-z-menu) cursor-default"
          />
          <div
            ref={card}
            role="dialog"
            aria-label={m["shell.overflow"]()}
            className="menu fixed top-[calc(var(--k7-header-phone-h)+12px)] right-4 z-(--k7-z-menu) flex max-h-[calc(100dvh-80px)] w-[300px] flex-col gap-1 overflow-y-auto rounded-[22px] p-3"
          >
            <Heading>{m["shell.overflow.pane"]()}</Heading>
            <div
              role="radiogroup"
              aria-label={m["shell.overflow.pane"]()}
              className="glass flex rounded-full p-[3px]"
            >
              {panes.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  role="radio"
                  aria-checked={pane === p.id}
                  aria-disabled={p.blocked ? true : undefined}
                  onClick={
                    p.blocked
                      ? run(() => showReason(p.blocked ?? ""))
                      : run(() => onPane(p.id))
                  }
                  className="type-label flex h-11 min-w-0 flex-1 items-center justify-center gap-1.5 rounded-full px-1 text-foreground aria-checked:bg-on-fill aria-checked:text-lavender aria-disabled:opacity-40"
                >
                  <p.icon className="size-[18px] shrink-0" />
                  <span className="truncate">{p.label}</span>
                </button>
              ))}
            </div>
            {lanes.length > 0 && (
              <>
                <Heading>{lanesHeading}</Heading>
                <div className="flex gap-1.5">
                  {lanes.map((l) => (
                    <button
                      key={l.id}
                      type="button"
                      aria-pressed={l.action ? undefined : l.on}
                      onClick={l.onPress}
                      className="type-label glass flex h-11 flex-1 items-center justify-center gap-1.5 rounded-full text-foreground aria-pressed:bg-on-fill aria-pressed:text-lavender"
                    >
                      <l.icon className="size-[18px]" />
                      {l.label}
                    </button>
                  ))}
                </div>
              </>
            )}
            {tool && (
              <>
                <Heading>{m["shell.overflow.tool"]()}</Heading>
                <div
                  role="radiogroup"
                  aria-label={m["shell.overflow.tool"]()}
                  className="glass flex rounded-full p-[3px]"
                >
                  {tool.tools.map((t) => (
                    <button
                      key={t}
                      type="button"
                      role="radio"
                      aria-checked={tool.current === t}
                      aria-label={TOOL_NAME[t]()}
                      title={TOOL_NAME[t]()}
                      onClick={run(() => tool.onPick(t))}
                      className="grid h-11 min-w-0 flex-1 place-items-center rounded-full text-foreground aria-checked:bg-on-fill aria-checked:text-lavender [&_svg]:size-[18px]"
                    >
                      <ToolGlyph tool={t} />
                    </button>
                  ))}
                </div>
              </>
            )}
            {metronome && (
              <div className="mt-1 flex flex-col gap-1">
                <button
                  type="button"
                  aria-pressed={metronome.on}
                  onClick={run(metronome.onPress)}
                  className="type-label glass flex h-11 items-center justify-center gap-1.5 rounded-full text-foreground aria-pressed:bg-on-fill aria-pressed:text-lavender"
                >
                  <MetronomeIcon className="size-[18px]" />
                  {m["transport.metronome"]()}
                </button>
                {/* The level the bar's right-click gives a Pad: committed on release, one command, one undo. */}
                <input
                  type="range"
                  min={METRONOME_MIN}
                  max={1}
                  step={0.05}
                  value={dragging ?? metronome.level}
                  onChange={(e) => setDragging(Number(e.target.value))}
                  onPointerUp={commitLevel}
                  onKeyUp={commitLevel}
                  onBlur={commitLevel}
                  aria-label={m["metro.volumeLabel"]()}
                  className="mx-3 h-11"
                />
              </div>
            )}
            <Heading>{m["shell.overflow.panels"]()}</Heading>
            <div className="grid grid-cols-3 gap-1.5">
              {panels.map((p) => {
                const def = PANEL_DEFS[p.id];
                return (
                  <button
                    key={p.id}
                    type="button"
                    aria-pressed={p.pressed}
                    disabled={p.disabled}
                    onClick={run(p.onPress)}
                    className={cn(
                      "type-secondary flex h-[68px] flex-col items-center justify-center gap-1 rounded-2xl bg-(--k7-row) px-1 text-foreground disabled:opacity-40",
                      p.pressed && "bg-on-fill text-lavender",
                    )}
                  >
                    <def.icon className="size-5" />
                    <span className="max-w-full truncate">{def.title()}</span>
                  </button>
                );
              })}
              <button
                type="button"
                onClick={run(onSettings)}
                className="type-secondary flex h-[68px] flex-col items-center justify-center gap-1 rounded-2xl bg-(--k7-row) px-1 text-foreground"
              >
                <Settings className="size-5" />
                <span className="max-w-full truncate">
                  {m["settings.menu"]()}
                </span>
              </button>
              <button
                type="button"
                onClick={run(onHelp)}
                className="type-secondary flex h-[68px] flex-col items-center justify-center gap-1 rounded-2xl bg-(--k7-row) px-1 text-foreground"
              >
                <CircleHelp className="size-5" />
                <span className="max-w-full truncate">{m["help.menu"]()}</span>
              </button>
            </div>
          </div>
        </>
      )}
    </>
  );
}
