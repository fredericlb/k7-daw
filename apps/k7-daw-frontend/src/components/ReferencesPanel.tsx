// ReferencesPanel.tsx — the workspace's reference tracks, as a panel on the right: what is in
// `_references/`, a ＋ that uploads into it, and a row you click to hear. It plays nothing
// itself — the player (ReferenceBar) does, drawing its card in this panel's slot (../docs/references.md).

import { askConfirm } from "@studio/project-shell/ask";
import {
  REFERENCE_ACCEPT,
  REFERENCES_DIR,
  type ReferenceTrack,
} from "@studio/song-schema/reference";
import { Button } from "@studio/ui/button";
import { EmptyState } from "@studio/ui/empty-state";
import { Music, Pause, Play, Plus, Trash2 } from "@studio/ui/icons";
import { Loading, Spinner } from "@studio/ui/spinner";
import { formatBytes } from "@studio/ui-chains-react/node-load";
import { byPeoplePath } from "@studio/web-protocol/tree";
import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { parts } from "../i18nParts";
import { m } from "../paraglide/messages.js";
import { PanelError } from "./PanelError";

export function ReferencesPanel({
  playing,
  sounding,
  onPlay,
  onToggle,
  playerSlot,
}: {
  /** The track the bar has loaded, by path — the row for it reads as the current one. */
  playing: string | null;
  /** Whether that loaded track is sounding: its row then reads Pause. */
  sounding: boolean;
  onPlay: (track: ReferenceTrack) => void;
  /** Receives the element the reference player's card is drawn into, above the list. */
  playerSlot?: (el: HTMLElement | null) => void;
  /** The bar's own play/pause, for the loaded track's row. */
  onToggle: () => void;
}) {
  const [rows, setRows] = useState<ReferenceTrack[] | null>(null);
  const [error, setError] = useState("");
  /** What an upload or a removal said, kept apart from a failed read: Retry only fits the second. */
  const [note, setNote] = useState("");
  /** What the upload is doing, as "file — 40%"; null when nothing is going up. */
  const [busy, setBusy] = useState<string | null>(null);
  const pick = useRef<HTMLInputElement>(null);

  const refresh = useCallback(async () => {
    try {
      // The service answers in code points; the list reads for people (../../docs/listOrder.md).
      setRows(
        [...(await api.references()).references].sort((a, b) =>
          byPeoplePath(a.path, b.path),
        ),
      );
      setError("");
    } catch (e) {
      setError((e as Error).message);
    }
  }, []);
  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** Upload each chosen file into the folder, then re-read: the LISTING is the truth, not what we sent. */
  const upload = async (files: FileList): Promise<void> => {
    setNote("");
    for (const file of Array.from(files)) {
      try {
        setBusy(file.name);
        await api.uploadWithProgress(
          REFERENCES_DIR,
          file.name,
          file,
          (loaded, total) =>
            setBusy(
              `${file.name} — ${Math.round((loaded / Math.max(1, total)) * 100)}%`,
            ),
        );
      } catch (e) {
        setNote(`${file.name}: ${(e as Error).message}`);
      }
    }
    setBusy(null);
    await refresh();
  };

  const remove = async (track: ReferenceTrack): Promise<void> => {
    const ok = await askConfirm({
      title: m["refs.removeTitle"](),
      body: m["refs.removeBody"]({ name: track.name }),
      confirmLabel: m["common.remove"](),
      cancelLabel: m["common.cancel"](),
      danger: true,
    });
    if (!ok) return;
    try {
      // Recycled rather than destroyed, like every other delete on this service.
      await api.deleteFile(track.path);
      await refresh();
    } catch (e) {
      setNote((e as Error).message);
    }
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <Button
        variant="outline"
        className="mb-2 w-full shrink-0 justify-center"
        disabled={busy !== null}
        onClick={() => pick.current?.click()}
      >
        <Plus className="size-4" /> {m["refs.add"]()}
      </Button>

      {/* The OS picker, hidden: `accept` is the same extension list the listing goes by. */}
      <input
        ref={pick}
        type="file"
        multiple
        accept={REFERENCE_ACCEPT}
        className="hidden"
        onChange={(e) => {
          const files = e.currentTarget.files;
          if (files?.length) void upload(files);
          e.currentTarget.value = "";
        }}
      />

      {busy ? (
        <p className="flex shrink-0 items-center gap-2 border-b bg-muted/30 px-2 py-1.5 text-muted-foreground text-xs">
          <Spinner size="xs" inherit className="shrink-0" /> {busy}
        </p>
      ) : null}

      {error ? (
        <PanelError
          what={m["panel.what.references"]()}
          detail={error}
          onRetry={() => void refresh()}
        />
      ) : null}

      {note ? (
        <p className="shrink-0 border-b px-2 py-1.5 text-error text-xs">
          {note}
        </p>
      ) : null}

      <div ref={playerSlot} className="shrink-0 empty:hidden" />

      <div className="min-h-0 flex-1 overflow-y-auto">
        {rows === null ? (
          <div className="p-4 text-muted-foreground text-sm">
            <Loading label={m["common.loading"]()} />
          </div>
        ) : rows.length === 0 ? (
          <EmptyState compact glyph={<Music />} title={m["refs.emptyTitle"]()}>
            {parts((h) => m["refs.empty"](h), {
              dir: <span className="font-mono">{REFERENCES_DIR}/</span>,
            })}
          </EmptyState>
        ) : (
          <ul>
            {rows.map((r) => (
              <li key={r.path} className="flex items-center gap-1 px-1">
                <button
                  type="button"
                  onClick={() => (playing === r.path ? onToggle() : onPlay(r))}
                  title={r.path}
                  aria-current={playing === r.path}
                  className={`flex min-w-0 flex-1 items-center gap-2 rounded px-1.5 py-1 text-left text-sm hover:bg-accent ${
                    playing === r.path ? "text-foreground" : ""
                  }`}
                >
                  {playing === r.path && sounding ? (
                    <Pause className="h-3 w-3 shrink-0 text-foreground" />
                  ) : (
                    <Play
                      className={`h-3 w-3 shrink-0 ${playing === r.path ? "text-foreground" : "text-muted-foreground"}`}
                    />
                  )}
                  <span className="min-w-0 flex-1 truncate">{r.name}</span>
                  <span className="shrink-0 font-mono text-[11px] text-muted-foreground tabular-nums">
                    {formatBytes(r.bytes)}
                  </span>
                </button>
                <Button
                  size="icon"
                  variant="ghost"
                  className="shrink-0"
                  title={m["refs.remove"]()}
                  aria-label={m["refs.remove"]()}
                  onClick={() => void remove(r)}
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
