// CommandsSettings.tsx — the "Commandes" settings screen: rename a command, give it an icon,
// bind it a keyboard shortcut, say which right-clicks offer it, and arrange the order menus list
// them in. Filtered by the ENTITY a command acts on. The catalogue itself is the app's; everything edited here is the person's
// own layer over it (commandPrefs.ts + keys.ts's effectiveKeys) and travels with their account.
// It edits a DRAFT and saves on a button — docs/shell.md says why this page is the exception.

import { askConfirm } from "@studio/project-shell/ask";
import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import {
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  Layers,
  Loader2,
  RotateCcw,
} from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { cn } from "@studio/ui/lib/utils";
import { useUnsavedGuard } from "@studio/ui-editor/unsaved";
import {
  lazy,
  memo,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useState,
} from "react";
import { entityName } from "../commandEntityNames";
import {
  type CommandPref,
  type CommandPrefs,
  commandHeads,
  entitiesOf,
  entityGroup,
  isPluralCommand,
  labelIn,
  labelOf,
  moveCommand,
  orderCommands,
  setCommandLabel,
  setCommandPref,
  setMenuEntities,
} from "../commandPrefs";
import { useWindowKey } from "../hooks/useWindowKey";
import { useLocale } from "../i18n";
import {
  effectiveKeys,
  KEY_NONE,
  keyForCommand,
  keyLabel,
  keyToken,
} from "../keys";
import { type IconSet, useIcons } from "../lucideIcons";
import { m } from "../paraglide/messages.js";
import { MENU_ENTITIES, type MenuEntity } from "../types";
import type { PaletteCommand } from "./CommandBar";
import { MenuEntityPicker } from "./CommandMenuPicker";

// The whole lucide set is ~1500 components, so it is LAZY: that cost is paid when this screen opens (see IconPicker's header).
const IconPicker = lazy(() => import("./IconPicker"));

/** One command's patch, as the row hands it up — the same shape `setCommandPref` takes. */
type Patch = Parameters<typeof setCommandPref>[2];

export function CommandsSettings({
  commands,
  prefs,
  onSave,
  onBack,
}: {
  /** The catalogue as the backend serves it — sid, line, and the sentence it is listed by. */
  commands: PaletteCommand[];
  /** The SAVED document. Everything on screen is a draft over it until the button is pressed. */
  prefs: CommandPrefs;
  /** Write the whole document. Called by the Save button and by nothing else. */
  onSave: (next: CommandPrefs) => void;
  onBack: () => void;
}) {
  const [entity, setEntity] = useState<string>("");
  const [q, setQ] = useState("");
  // Every language the app is built with, so a new one grows a field here — and the CURRENT one, to mark which you are reading in.
  const { locale, locales } = useLocale();
  // Shared with the context menus, so this page and they resolve a name the same way; null for the first frames.
  const icons = useIcons();
  const entities = useMemo(() => entitiesOf(commands), [commands]);
  // Every head the catalogue uses, so a PLURAL verb lists under its singular — see entityGroup.
  const heads = useMemo(() => commandHeads(commands), [commands]);

  /** The edits, held HERE until the Save button — a write-through is a PATCH and a whole-shell re-render per character (docs/shell.md). */
  const [draft, setDraft] = useState<CommandPrefs>(prefs);
  const dirty = draft !== prefs;
  // `leave` below guards the in-app road; this one guards the reload, which was the way out with no question.
  useUnsavedGuard(dirty);
  // The saved document changed under us: the first fetch landing, or our own save coming back, which costs a bail-out.
  useEffect(() => {
    setDraft(prefs);
  }, [prefs]);

  // Stable, all three: the rows are memoised on them, which functional updates are what make possible.
  const patch = useCallback(
    (canonical: string, p: Patch) =>
      setDraft((d) => setCommandPref(d, canonical, p)),
    [],
  );
  const rename = useCallback(
    (canonical: string, l: string, text: string) =>
      setDraft((d) => setCommandLabel(d, canonical, l, text)),
    [],
  );
  const move = useCallback(
    (canonical: string, by: -1 | 1) =>
      setDraft((d) => moveCommand(commands, d, canonical, by)),
    [commands],
  );
  const menu = useCallback(
    (
      canonical: string,
      catalogue: readonly MenuEntity[] | undefined,
      next: readonly MenuEntity[],
    ) =>
      setDraft((d) =>
        setMenuEntities(d, canonical, catalogue, next, MENU_ENTITIES),
      ),
    [],
  );

  // Ordered FIRST, then filtered — so the arrows move a command against the whole arrangement, not against what the filter shows.
  const ordered = useMemo(
    () => orderCommands(commands, draft),
    [commands, draft],
  );
  const needle = q.trim().toLowerCase();
  const shown = ordered.filter(
    (c) =>
      (!entity || entityGroup(c.canonical, heads) === entity) &&
      (!needle ||
        c.canonical.toLowerCase().includes(needle) ||
        labelOf(c, draft, locale).toLowerCase().includes(needle)),
  );

  /** The shortcut each row shows, resolved ONCE for the whole page: per row it rebuilt the entire effective map, two hundred times a keystroke. */
  const keyByCommand = useMemo(() => {
    const out: Record<string, string> = {};
    for (const [token, canonical] of Object.entries(effectiveKeys(draft)))
      if (!(canonical in out)) out[canonical] = keyLabel(token);
    return out;
  }, [draft]);

  // The row whose shortcut cell is waiting for a key, by canonical. One at a time, obviously.
  const [capturing, setCapturing] = useState<string | null>(null);
  // What the last bound key took from another command, said once on the status line.
  const [took, setTook] = useState("");
  /** The capture itself, on the window and in the capture phase; Escape renounces, and Backspace takes the shortcut away as KEY_NONE. */
  useWindowKey(
    (e) => {
      if (!capturing) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        setCapturing(null);
        return;
      }
      if (["Shift", "Control", "Alt", "Meta"].includes(e.key)) return; // still choosing
      if (e.key === "Backspace" || e.key === "Delete") {
        patch(capturing, {
          key: keyForCommand(capturing) ? KEY_NONE : undefined,
        });
        setCapturing(null);
        return;
      }
      const token = keyToken(e);
      // A modifier chord stays the browser's — ⌃C and ⌃V are the two exceptions, being the shell's own keys (keys.ts's CHORDS).
      if (!token) return;
      const holder = effectiveKeys(draft)[token];
      patch(capturing, {
        key: token === keyForCommand(capturing) ? undefined : token,
      });
      setTook(
        holder && holder !== capturing
          ? m["cmds.tookKey"]({ key: keyLabel(token), command: holder })
          : "",
      );
      setCapturing(null);
    },
    { enabled: !!capturing, capture: true },
  );

  // Ctrl+S means the button, as in the module builder; not while capturing, since that effect owns the keyboard.
  useWindowKey(
    (e) => {
      if (!(e.key === "s" && (e.ctrlKey || e.metaKey))) return;
      e.preventDefault();
      if (dirty) onSave(draft);
    },
    { enabled: !capturing },
  );

  // Leaving with edits nobody asked to keep is the one thing a Save button makes possible, so it is the one thing this stops.
  const leave = async () => {
    if (!dirty) return onBack();
    const agreed = await askConfirm({
      title: m["cmds.discardTitle"](),
      body: m["cmds.discardBody"](),
      confirmLabel: m["cmds.discard"](),
      danger: true,
    });
    if (agreed) onBack();
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex shrink-0 flex-wrap items-center gap-3 border-b border-(--k7-hairline) px-4 py-2">
        <Button size="sm" variant="ghost" onClick={() => void leave()}>
          <ArrowLeft className="size-4" /> {m["common.back"]()}
        </Button>
        <h2 className="type-sheet-title">{m["cmds.title"]()}</h2>
        <span className="hidden type-secondary text-fg-2 md:inline">
          {m["cmds.subtitle"]()}
        </span>
        <Input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder={m["common.filter"]()}
          aria-label={m["common.filter"]()}
          className="ml-auto w-full sm:w-56"
        />
        {/* The button, and beside it what it would write: a shortcut assigned here does not run until it is saved. */}
        <span
          className={cn(
            "shrink-0 type-secondary",
            dirty ? "text-warn" : "text-fg-2",
          )}
        >
          {dirty ? m["cmds.unsaved"]() : m["cmds.upToDate"]()}
        </span>
        <Button
          size="sm"
          variant="ghost"
          disabled={!dirty}
          onClick={() => setDraft(prefs)}
        >
          {m["common.cancel"]()}
        </Button>
        <Button size="sm" disabled={!dirty} onClick={() => onSave(draft)}>
          {m["common.save"]()}
        </Button>
      </header>

      {/* The chips: what a command acts on, read off its sid. */}
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-(--k7-hairline) px-4 py-2">
        {["", ...entities].map((e) => (
          <Chip
            variant="filter"
            key={e || "all"}
            selected={entity === e}
            onClick={() => setEntity(e)}
          >
            {e ? entityName(e) : m["cmds.all"]()}
          </Chip>
        ))}
        <span className="ml-auto type-secondary text-fg-2">
          {shown.length}/{commands.length}
        </span>
      </div>
      <div
        aria-hidden
        className="hidden shrink-0 items-center gap-2 border-b border-(--k7-hairline) px-4 py-1.5 type-badge text-fg-3 uppercase md:flex"
      >
        <span className="w-9 shrink-0">{m["cmds.colIcon"]()}</span>
        {locales.map((l) => (
          <span key={l} className="min-w-40 flex-1">
            {m["cmds.colName"]({ locale: l })}
          </span>
        ))}
        <span className="w-56 shrink-0">{m["cmds.colCommand"]()}</span>
        <span className="w-28 shrink-0">{m["cmds.colMenu"]()}</span>
        <span className="hidden w-[76px] shrink-0 [@media(any-pointer:fine)]:inline">
          {m["cmds.colKey"]()}
        </span>
        <span className="w-36 shrink-0">{m["cmds.colOrder"]()}</span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        {shown.map((c) => (
          <CommandRow
            key={c.canonical}
            c={c}
            pref={draft[c.canonical]}
            icons={icons}
            locale={locale}
            locales={locales}
            plural={isPluralCommand(c.canonical, heads)}
            keyText={keyByCommand[c.canonical] ?? null}
            capturing={capturing === c.canonical}
            onCapture={setCapturing}
            onPatch={patch}
            onRename={rename}
            onMove={move}
            onMenu={menu}
          />
        ))}
        {shown.length === 0 && (
          <p className="py-8 text-center text-sm text-fg-2">
            {m["cmds.noMatch"]()}
          </p>
        )}
      </div>

      {/* One status line: the capture hint while a key is awaited, else what the last bound key took. */}
      <p
        role="status"
        className={cn(
          "min-h-8 shrink-0 border-t border-(--k7-hairline) px-4 py-1.5 type-secondary",
          capturing ? "text-fg-2" : "text-warn",
        )}
      >
        {capturing ? m["cmds.captureHint"]() : took}
      </p>
    </div>
  );
}

/** One command's row — memoised, which is the second half of what made this page usable: a keystroke reconciled two hundred rows. */
const CommandRow = memo(function CommandRow({
  c,
  pref,
  icons,
  locale,
  locales,
  plural,
  keyText,
  capturing,
  onCapture,
  onPatch,
  onRename,
  onMove,
  onMenu,
}: {
  c: PaletteCommand;
  pref: CommandPref | undefined;
  icons: IconSet | null;
  locale: string;
  locales: readonly string[];
  plural: boolean;
  /** Already resolved against the whole page — see `keyByCommand`. */
  keyText: string | null;
  capturing: boolean;
  onCapture: (canonical: string | null) => void;
  onPatch: (canonical: string, patch: Patch) => void;
  onRename: (canonical: string, locale: string, text: string) => void;
  onMove: (canonical: string, by: -1 | 1) => void;
  /** The whole list at once — a pick is "these ones", never a diff. */
  onMenu: (
    canonical: string,
    catalogue: readonly MenuEntity[] | undefined,
    next: readonly MenuEntity[],
  ) => void;
}) {
  const name = pref?.icon;
  const Icon = name ? icons?.[name] : undefined;
  const touched = !!pref;
  return (
    <div className="relative flex flex-wrap items-center gap-2 border-b border-(--k7-hairline) px-4 py-2">
      {/* The icon, and the picker behind it. */}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            type="button"
            title={m["cmds.pickIcon"]()}
            className="flex size-9 shrink-0 cursor-pointer items-center justify-center rounded-xl border border-(--k7-hairline-strong) text-fg-2 hover:text-foreground"
          >
            {Icon ? <Icon className="size-4" /> : "—"}
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="p-2">
          <Suspense
            fallback={
              <div className="flex h-24 w-72 items-center justify-center">
                <Loader2 className="size-4 animate-spin" />
              </div>
            }
          >
            <IconPicker
              value={pref?.icon}
              onPick={(icon) => onPatch(c.canonical, { icon })}
            />
          </Suspense>
        </DropdownMenuContent>
      </DropdownMenu>

      {/* The name, one field per language, each placeholder the catalogue's own sentence IN THAT language (docs/shell.md). */}
      {locales.map((l) => (
        <label
          key={l}
          className="flex min-w-40 flex-1 items-center gap-1"
          title={m["cmds.nameIn"]({ locale: l })}
        >
          <span
            className={cn(
              "shrink-0 font-mono text-[10px] uppercase",
              l === locale ? "text-foreground" : "text-faint",
            )}
          >
            {l}
          </span>
          <Input
            value={labelIn(pref?.label, l)}
            placeholder={c.readableBy?.[l] || c.readable || c.canonical}
            onChange={(e) => onRename(c.canonical, l, e.target.value)}
            className="min-w-0 flex-1"
          />
        </label>
      ))}
      {/* The line, with a mark on the one that acts on SEVERAL: a plural verb lists under its singular, where anyone would look. */}
      <code className="flex w-56 shrink-0 items-center gap-1 truncate type-mono-value text-fg-2">
        {plural && (
          <Layers
            className="size-3 shrink-0 text-primary-text/70"
            aria-label={m["cmds.plural"]()}
          />
        )}
        <span className="truncate">{c.canonical}</span>
      </code>

      {/* WHICH right-clicks offer it — the catalogue's own list until you touch it. */}
      <MenuEntityPicker c={c} pref={pref} onMenu={onMenu} />

      {/* The shortcut: click, press a key (hidden where no key can be pressed: a touch screen alone), and a stolen one goes blank in the row that held it, both reading one effective map. */}
      <Button
        size="sm"
        variant={capturing ? "default" : "outline"}
        title={m["cmds.keyHint"]()}
        aria-pressed={capturing}
        className="hidden h-9 w-[76px] shrink-0 justify-center type-mono-value [@media(any-pointer:fine)]:inline-flex"
        onClick={() => onCapture(capturing ? null : c.canonical)}
      >
        {capturing ? "…" : (keyText ?? "—")}
      </Button>

      <Button
        size="icon"
        variant="ghost"
        title={m["cmds.up"]()}
        onClick={() => onMove(c.canonical, -1)}
      >
        <ArrowUp className="size-3.5" />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        title={m["cmds.down"]()}
        onClick={() => onMove(c.canonical, 1)}
      >
        <ArrowDown className="size-3.5" />
      </Button>
      <Button
        size="icon"
        variant="ghost"
        disabled={!touched}
        title={m["cmds.giveBack"]()}
        onClick={() =>
          onPatch(c.canonical, {
            label: undefined,
            icon: undefined,
            order: undefined,
            key: undefined,
            menu: undefined,
          })
        }
      >
        <RotateCcw className={cn("size-3.5", !touched && "opacity-30")} />
      </Button>
    </div>
  );
});
