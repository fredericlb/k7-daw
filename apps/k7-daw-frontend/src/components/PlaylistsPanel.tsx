// PlaylistsPanel.tsx — the workspace's set lists as a PANEL anchored at the right: a searchable
// dropdown naming one, and its songs under it. Docked beside the panes rather than covering
// them, because it is something you keep open while you work.
//
// Its OWN panel and not a tab of the settings one: settings are things you set and leave, and a
// set list is a thing you work FROM — you open it to put a record on, not to configure
// anything. It also has the one header that is not
// a title (a dropdown, a ＋ and the text button), which a shared tab strip has nowhere to put.
// See ../docs/playlists.md.

import { playlistPathOf } from "@studio/k7-wasm/schema";
import { Button } from "@studio/ui/button";
import { Maximize2, Plus, TriangleAlert } from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { Picker } from "@studio/ui/picker";
import { Loading } from "@studio/ui/spinner";
import type { PlaylistInfo } from "@studio/web-protocol/playlists";
import { sortForPeople } from "@studio/web-protocol/tree";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { parts } from "../i18nParts";
import { m } from "../paraglide/messages.js";
import { PanelError } from "./PanelError";
import { PlaylistEditor } from "./PlaylistEditor";

export function PlaylistsPanel({
  vimMode,
  onRun,
}: {
  vimMode: boolean;
  /** Run a command line through the shell — what « Nouvelle playlist » dispatches, so the palette reaches it (../docs/shell.md). */
  onRun: (line: string, outcome?: { ok: boolean }) => Promise<void>;
}) {
  const [rows, setRows] = useState<PlaylistInfo[] | null>(null);
  const [error, setError] = useState("");
  /** Which one the panel is showing, by PATH — the listing is re-read on every save, so a row object would be a fresh identity. */
  const [open, setOpen] = useState<string | null>(null);
  /** The FULL editor, over the shell: the panel is too narrow for the text, and the page is the escape hatch for what it cannot edit (../docs/playlists.md). */
  const [expanded, setExpanded] = useState(false);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const nameRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (creating) nameRef.current?.focus();
  }, [creating]);

  // Answers the listing as well as storing it: `create` has to know the playlist is really there before opening it.
  const refresh = useCallback(
    (): Promise<PlaylistInfo[] | null> =>
      api
        .playlists()
        .then((r) => {
          setRows(r.playlists);
          setError("");
          return r.playlists;
        })
        .catch((e: Error) => {
          setError(e.message);
          return null;
        }),
    [],
  );
  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** Land on one as soon as there is one to land on — only while nothing is chosen, so a re-read never moves you off. */
  useEffect(() => {
    if (open === null && rows?.length) setOpen(rows[0].path);
  }, [rows, open]);

  // By what each row reads as, for people: the service answers by path, in code points (../../docs/listOrder.md).
  const items = useMemo(
    () =>
      sortForPeople(rows ?? [], (p) => p.title || p.name).map((p) => ({
        value: p.path,
        label: p.title || p.name,
        detail: p.path,
      })),
    [rows],
  );
  const current = rows?.find((p) => p.path === open) ?? null;

  /** `playlist new <name>` — the button dispatches the command, and `playlistPathOf` derives where it lands, a command answering in prose. */
  const create = useCallback(() => {
    const want = name.trim();
    if (!want) return;
    const rel = playlistPathOf(want);
    const outcome = { ok: false };
    void onRun(`playlist new ${want}`, outcome)
      .then(() => refresh())
      .then((list) => {
        // A refused create keeps the form and the typed name, the refusal being already told.
        if (!outcome.ok) return;
        setCreating(false);
        setName("");
        // Straight into it — but only if it is really there: a refused create must not show a panel for a file nobody wrote.
        if (rel && list?.some((p) => p.path === rel)) setOpen(rel);
      });
  }, [name, onRun, refresh]);

  if (expanded && open)
    return (
      <div className="absolute inset-0 z-40 bg-ground">
        <PlaylistEditor
          path={open}
          name={current?.title || current?.name || open}
          vimMode={vimMode}
          onBack={() => {
            setExpanded(false);
            void refresh();
          }}
          onSaved={() => void refresh()}
          onRun={onRun}
        />
      </div>
    );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex shrink-0 items-center gap-2 pb-2">
        <Picker
          className="min-w-0 flex-1"
          items={items}
          value={open}
          onValueChange={setOpen}
          label={m["pls.search"]()}
          searchLabel={m["pls.search"]()}
          placeholder={rows === null ? m["pls.loading"]() : m["pls.none"]()}
          emptyLabel={m["pls.none"]()}
        />
        <Button
          size="round"
          variant="glass"
          aria-pressed={creating}
          title={m["pls.new"]()}
          aria-label={m["pls.new"]()}
          onClick={() => setCreating((c) => !c)}
        >
          <Plus className="size-5" />
        </Button>
        {/* The text, when the panel cannot do what you need — see `expanded`. */}
        <Button
          size="round"
          variant="glass"
          disabled={!open}
          title={m["pls.editFile"]()}
          aria-label={m["pls.editFileShort"]()}
          onClick={() => setExpanded(true)}
        >
          <Maximize2 className="size-5" />
        </Button>
      </header>

      {creating ? (
        <div className="mb-2 flex shrink-0 items-center gap-2">
          {/* Focused through a ref rather than `autoFocus`, which steals focus on MOUNT — not the same event as the opening press. */}
          <Input
            ref={nameRef}
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => {
              e.stopPropagation(); // the shell binds bare keys; a field keeps its own
              if (e.key === "Escape") setCreating(false);
              if (e.key === "Enter") create();
            }}
            placeholder={m["pls.namePlaceholder"]()}
            className="min-w-0 flex-1"
          />
          <Button size="sm" onClick={create} disabled={!name.trim()}>
            {m["common.create"]()}
          </Button>
        </div>
      ) : null}

      {error ? (
        <PanelError
          what={m["panel.what.playlists"]()}
          detail={error}
          onRetry={() => void refresh()}
        />
      ) : null}

      {current && !current.ok ? (
        <p className="type-status flex shrink-0 items-center gap-1.5 pb-2 text-error">
          <TriangleAlert className="h-3 w-3 shrink-0" /> {m["pls.brokenFile"]()}
        </p>
      ) : null}

      {/* The songs, under the dropdown: `PlaylistEditor` in panel mode, the same document and autosave with the text half left out. */}
      <div className="min-h-0 flex-1">
        {rows === null ? (
          <div className="p-4 text-sm text-fg-2">
            <Loading label={m["pls.loading"]()} />
          </div>
        ) : rows.length === 0 ? (
          <p className="p-3 text-sm text-fg-2">
            {parts((h) => m["pls.emptyRepo"](h), {
              dir: <span className="font-mono">_playlists/</span>,
            })}
          </p>
        ) : open ? (
          <PlaylistEditor
            key={open}
            path={open}
            name={current?.title || current?.name || open}
            vimMode={vimMode}
            panel
            onSaved={() => void refresh()}
            onRun={onRun}
          />
        ) : null}
      </div>
    </div>
  );
}
