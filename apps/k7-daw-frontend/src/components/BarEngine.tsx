// BarEngine.tsx — the engine slot of a bar, the project's and the home pages': an icon button whose
// dot is the engine's state, opening what it says, the engine's settings, and where this device
// hears it. The engine is this device's own; the words are engineWords.tsx's. See ../../docs/shell.md.

import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { Circle, CircleDot, Settings } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { go } from "../appNav";
import { setListenHere, useListenHere } from "../listenHere";
import { m } from "../paraglide/messages.js";
import { ENGINE_SETTINGS_ROUTE } from "../projects/api";
import { type EngineSelectorProps, engineWords } from "./engineWords";

/** The dot on the button: the link is up, down, there is no engine, or one is still looked for. */
export type EngineStatus = "up" | "down" | "none" | "searching";

const DOT: Record<EngineStatus, string> = {
  up: "bg-emerald-500",
  down: "bg-red-500",
  none: "bg-amber-500",
  searching: "animate-pulse bg-muted-foreground",
};

export function BarEngine({
  current,
  searching,
  audioToBrowser = false,
  stats,
  status,
  onOpen,
  toolSize,
  align = "end",
}: EngineSelectorProps & {
  status: EngineStatus;
  /** The menu was opened: an engine that was not there yet is looked for again. */
  onOpen?: () => void;
  /** The bar's button size, so the icon matches the pages' buttons beside it. */
  toolSize: "icon" | "icon-xs";
  /** The menu's edge against the button's: the end at a bar's right end, the start in a sidebar. */
  align?: "start" | "end";
}) {
  const listenHere = useListenHere();
  // An engine with no speakers is heard here whatever the radio says.
  const here = listenHere || audioToBrowser;
  const { label, hint, icon } = engineWords({
    current,
    searching,
    audioToBrowser: here,
  });
  const statusText =
    status === "up"
      ? m["engineSwitch.linkUp"]()
      : status === "down"
        ? m["engineSwitch.linkDown"]()
        : label;
  return (
    <DropdownMenu
      onOpenChange={(open) => {
        if (open) onOpen?.();
      }}
    >
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size={toolSize}
          className="relative shrink-0"
          title={`${label} — ${statusText}`}
          aria-label={m["engineSwitch.section"]()}
        >
          {icon}
          {/* The engine's state at a glance; the tooltip and the menu say it in words. */}
          <span
            aria-hidden="true"
            className={cn(
              "absolute right-0.5 bottom-0.5 h-2 w-2 rounded-full ring-2 ring-card",
              DOT[status],
            )}
          />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        side="top"
        align={align}
        className="w-80 max-w-[calc(100vw-2rem)]"
      >
        <div className="flex flex-col gap-1 px-2 py-1.5 text-xs" title={hint}>
          <div className="flex min-w-0 items-center gap-1.5">
            {icon}
            <span className="min-w-0 flex-1 truncate font-medium text-sm">
              {label}
            </span>
          </div>
          {stats && <div className="flex items-center gap-1">{stats}</div>}
        </div>
        <DropdownMenuItem onSelect={() => void go(ENGINE_SETTINGS_ROUTE)}>
          <Settings className="h-4 w-4 text-muted-foreground" />
          {m["engineSwitch.settings"]()}
        </DropdownMenuItem>

        <DropdownMenuSeparator />
        <DropdownMenuLabel>{m["engineSwitch.playsOn"]()}</DropdownMenuLabel>
        <ListenRadio
          checked={here}
          label={m["engineSwitch.thisMachine"]()}
          hint={m["engineSwitch.thisMachineHint"]()}
          onPick={() => setListenHere(true)}
        />
        <ListenRadio
          checked={!here}
          label={m["engineSwitch.theEngine"]()}
          hint={
            audioToBrowser
              ? m["engineSwitch.noSpeakers"]()
              : m["engineSwitch.theEngineHint"]()
          }
          disabled={audioToBrowser}
          onPick={() => setListenHere(false)}
        />
        {/* A disabled item shows no tooltip, so the reason is written out. */}
        {audioToBrowser && (
          <p className="px-2 pb-1.5 text-muted-foreground text-xs">
            {m["engineSwitch.noSpeakers"]()}
          </p>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

/** A radio as a menu item: choosing it keeps the menu open, so the change is seen. */
function ListenRadio({
  checked,
  label,
  hint,
  disabled,
  onPick,
}: {
  checked: boolean;
  label: string;
  hint: string;
  disabled?: boolean;
  onPick: () => void;
}) {
  const Dot = checked ? CircleDot : Circle;
  return (
    <DropdownMenuItem
      role="menuitemradio"
      aria-checked={checked}
      disabled={disabled}
      title={hint}
      onSelect={(e) => {
        e.preventDefault();
        onPick();
      }}
    >
      <Dot
        className={cn(
          "h-4 w-4 shrink-0",
          checked ? "text-primary-text" : "text-muted-foreground",
        )}
      />
      {label}
    </DropdownMenuItem>
  );
}
