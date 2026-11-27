// CommandParams.tsx — the parameter form, as ONE row in the bar's own place: the command, then a field per param.
//
// Every question is on screen at once, so an answer given three fields ago can be changed without
// unwinding the ones after it; a field's suggestions open downward from the row, as the command list does.
//
// It holds NO state: CommandBar owns the command, the answers and the gesture inlets, and two
// copies of that would be two commands. See docs/commandBar.md.

import type { ArgOption, EnterAction } from "@studio/k7-wasm/ui";
import { Button } from "@studio/ui/button";
import { cn } from "@studio/ui/lib/utils";
import { useId } from "react";
import { m } from "../paraglide/messages.js";
import type { ArgClass } from "../types";
import type { PaletteArg, PaletteCommand } from "./CommandBar";

/** A field's width in characters: what it holds, else its placeholder, so the row reads like the line it builds. */
const fieldSize = (value: string, hint: string): number =>
  Math.max(3, Math.min(32, (value || hint).length + 1));

export function CommandParams({
  command,
  values,
  active,
  suggestions,
  selected,
  argColor,
  enterDoes,
  onType,
  onFocusField,
  onPick,
  onHover,
  onKeyDown,
  onRepick,
  onRun,
  canRun,
  inputRef,
}: {
  command: PaletteCommand;
  values: readonly string[];
  active: number;
  /** The active field's vocabulary, already ranked — empty when it has none. */
  suggestions: readonly ArgOption[];
  selected: number;
  argColor: Record<ArgClass, string>;
  /** What Enter will do THIS press, so the row can name it — see `enterAction`. */
  enterDoes: EnterAction["kind"];
  onType: (index: number, value: string) => void;
  onFocusField: (index: number) => void;
  onPick: (value: string) => void;
  onHover: (index: number) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  /** The command's name was clicked: drop it and pick another. */
  onRepick: () => void;
  /** Run the line as it stands — the panel's Run button. */
  onRun: () => void;
  /** Whether every required field is answered. */
  canRun: boolean;
  inputRef: React.RefObject<HTMLInputElement | null>;
}) {
  const arg: PaletteArg | undefined = command.args[active];
  const listId = useId();
  const picked = suggestions.find((o) => o.value === (values[active] ?? ""));

  return (
    <div className="flex h-5 min-w-0 flex-1 items-center gap-1.5 overflow-hidden pointer-coarse:h-11">
      {/* The NAME, which is the line being built; the sentence is its title, reading back what was chosen. */}
      <button
        type="button"
        onClick={onRepick}
        className="h-5 shrink-0 font-mono text-sm leading-5 text-foreground hover:text-primary-text pointer-coarse:h-11"
        title={m["commandBar.changeTitle"]({ readable: command.readable })}
        aria-label={m["commandBar.changeNamed"]({
          command: command.canonical,
        })}
      >
        {command.canonical}
      </button>
      <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto [scrollbar-width:none]">
        {command.args.map((a, i) => (
          <input
            key={a.name}
            ref={i === active ? inputRef : undefined}
            value={values[i] ?? ""}
            onChange={(e) => onType(i, e.target.value)}
            onFocus={() => onFocusField(i)}
            onKeyDown={onKeyDown}
            aria-label={a.name}
            // The active field is the listbox's combobox, as the command field is.
            role={i === active ? "combobox" : undefined}
            aria-expanded={i === active ? suggestions.length > 0 : undefined}
            aria-controls={i === active ? listId : undefined}
            aria-activedescendant={
              i === active && suggestions.length > 0
                ? `${listId}-${selected}`
                : undefined
            }
            // The param's name is the placeholder, a trailing optional marked so: leaving it blank is an ANSWER.
            placeholder={a.optional ? `${a.name}?` : a.name}
            size={fieldSize(values[i] ?? "", a.name)}
            title={
              i === active && picked?.label
                ? `${a.name}: ${picked.label}`
                : (a.hint ?? a.name)
            }
            spellCheck={false}
            autoComplete="off"
            className={cn(
              // h-5, the bar input's own line: a field any taller makes the whole strip jump when a command is picked.
              "h-5 shrink-0 rounded border bg-background/60 px-1.5 py-0 font-mono text-xs leading-none outline-none focus:border-primary placeholder:text-faint pointer-coarse:h-11 pointer-coarse:px-3 pointer-coarse:text-[16px]",
              i === active
                ? `border-primary/60 ${argColor[a.kind]}`
                : "border-border text-foreground/80",
            )}
          />
        ))}
      </div>
      {/* What Enter does THIS press, which is not always "run" (see `enterAction`). */}
      <span
        className={cn(
          "type-badge shrink-0 text-muted-foreground pointer-coarse:hidden",
          enterDoes === "run" && "text-foreground",
        )}
      >
        {enterDoes === "take"
          ? m["commandBar.enterPick"]()
          : enterDoes === "run"
            ? m["commandBar.enterRun"]()
            : m["commandBar.enterNext"]()}
      </span>

      {/* Opens DOWNWARD, as the command list does: the sentence, the active field's hint, its values, then Back and Run. */}
      <div className="menu absolute top-full right-0 left-0 z-30 mt-2 flex flex-col rounded-(--k7-r-menu) py-1.5 text-xs">
        <p className="truncate px-3 pt-1 font-bold type-secondary text-foreground">
          {command.readable}
        </p>
        {arg?.hint && (
          <p className="truncate px-3 pb-1 type-status text-fg-3">
            {arg.name}: {arg.hint}
          </p>
        )}
        {suggestions.length > 0 && (
          <>
            <p className="type-caps px-3 pt-1 pb-0.5 text-fg-3">{arg?.name}</p>
            <div
              id={listId}
              role="listbox"
              aria-label={arg?.name}
              className="flex max-h-60 flex-col overflow-auto"
            >
              {suggestions.map((o, i) => (
                <div
                  key={o.value}
                  id={`${listId}-${i}`}
                  role="option"
                  aria-selected={i === selected}
                  ref={
                    i === selected
                      ? (el) => el?.scrollIntoView({ block: "nearest" })
                      : undefined
                  }
                  // mousedown, and preventDefault: the blur that would close this list must never be the one that was about to pick from it.
                  onMouseDown={(e) => {
                    e.preventDefault();
                    onPick(o.value);
                  }}
                  onMouseEnter={() => onHover(i)}
                  className={cn(
                    "w-full cursor-pointer px-3 py-0.5 text-left font-mono pointer-coarse:flex pointer-coarse:min-h-11 pointer-coarse:items-center pointer-coarse:py-2",
                    arg ? argColor[arg.kind] : "text-foreground",
                    i === selected && "bg-on-fill",
                  )}
                >
                  {/* A labelled option is LISTED by its label and inserts its value, which a person cannot read (commands/docs/completions.md). */}
                  {o.label ?? o.value}
                </div>
              ))}
            </div>
          </>
        )}
        <p
          aria-hidden
          className="type-badge px-3 pt-1 text-fg-3 pointer-coarse:hidden"
        >
          {m["commandBar.footerArgs"]()}
        </p>
        <div className="flex justify-end gap-2 px-2 pt-1">
          <Button
            size="sm"
            variant="ghost"
            className="pointer-coarse:h-11"
            onMouseDown={(e) => e.preventDefault()}
            onClick={onRepick}
          >
            {m["common.back"]()}
          </Button>
          <Button
            size="sm"
            className="pointer-coarse:h-11"
            disabled={!canRun}
            onMouseDown={(e) => e.preventDefault()}
            onClick={onRun}
          >
            {m["commandBar.run"]()}
          </Button>
        </div>
      </div>
    </div>
  );
}
