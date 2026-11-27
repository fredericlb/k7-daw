// PlaylistPane.tsx — the right half of a playlist's screen: its lines resolved against the
// workspace's audio files, each one playable, and one way of putting another one there — search
// those files. The lines' rules are `@studio/song-schema/records`, shared with the DJ desk, which
// resolves the same lines. See ../docs/playlists.md.

import { mediaUrl } from "@studio/project-shell/fileApi";
import type { PlaylistItem } from "@studio/k7-wasm/types";
import {
  packRecordDrag,
  RECORD_DRAG_TYPE,
  type ResolvedRecord,
  recordFolder,
  recordMatches,
  recordName,
  resolveRecords,
} from "@studio/song-schema/records";
import { Button } from "@studio/ui/button";
import {
  ChevronDown,
  ChevronUp,
  Disc3,
  ListMusic,
  Pause,
  Play,
  Plus,
  Search,
  Split,
  X,
} from "@studio/ui/icons";
import { Input } from "@studio/ui/input";
import { cn } from "@studio/ui/lib/utils";
import { Row, RowButton, RowGroup } from "@studio/ui/row-group";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { parts } from "../i18nParts";
import { m } from "../paraglide/messages.js";
import { playlistVocabulary } from "../playlistVocabulary";
import { type Run, StemsBar } from "./StemsBar";

/** How many search hits are drawn at once: three hundred records in one scrolling box is not a list you read, so the field is the way through. */
const MAX_HITS = 40;

/** What a row is called: the file's name, else the line as written. */
const labelOf = (r: ResolvedRecord): string =>
  r.path ? recordName(r.path) : r.item.song;

export function PlaylistPane({
  items,
  files,
  playlist,
  onAdd,
  addRefusal,
  onRemove,
  onMove,
  editRefusal,
  onRun,
}: {
  /** The lines, as the text on the left currently parses. */
  items: PlaylistItem[];
  /** The workspace's audio files, `stems/` included, or null before the tree has arrived. */
  files: ReadonlySet<string> | null;
  /** The playlist's name, which `playlist stems` takes. */
  playlist: string;
  /** Put a line at the end of the playlist: a request upwards rather than a write, since the EDITOR owns the text. */
  onAdd: (item: PlaylistItem) => void;
  /** Why `onAdd` would not work right now — the file's `items:` is in a shape the text-level append cannot extend. */
  addRefusal?: string;
  /** Drop the line at a position — upwards for the same reason `onAdd` is; absent drops the buttons. */
  onRemove?: (index: number) => void;
  /** Move a line, both positions 0-based — the pane counts from 1 only where it PRINTS. */
  onMove?: (from: number, to: number) => void;
  /** Why those two would not work — the same kind of fact as `addRefusal`, about a shape text cannot reorder. */
  editRefusal?: string;
  /** Dispatch a command line — the stem bar sends you to the settings with one; absent drops the button and keeps the sentence. */
  onRun?: (line: string, outcome?: { ok: boolean }) => Promise<void>;
}) {
  const rows = useMemo(() => resolveRecords(items, files), [items, files]);

  /** The transport, and the whole of it: one element playing one file, this shell already having a transport of its own. */
  const audioRef = useRef<HTMLAudioElement | null>(null);
  /** Which line this pane started, or null. */
  const [current, setCurrent] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  /** What the footer says about a click that did not start anything. Cleared by the next one. */
  const [notice, setNotice] = useState("");

  /** The record's file, when the workspace holds it. */
  const fileOf = useCallback(
    (r: ResolvedRecord): string | null => (r.present ? r.path : null),
    [],
  );

  /** Start the first line from `from` that has audio, or stop if none has — skipping only ever over a RESOLUTION problem. */
  const startAt = useCallback(
    (from: number) => {
      setNotice("");
      for (let i = from; i < rows.length; i++) {
        const file = fileOf(rows[i] as ResolvedRecord);
        if (!file) continue;
        const el = audioRef.current;
        if (!el) return;
        el.src = mediaUrl(file);
        setCurrent(i);
        void el.play().catch((e: Error) => setNotice(e.message));
        return;
      }
      setCurrent(null);
      if (from === 0) setNotice(m["pl.noAudioRow"]());
    },
    [rows, fileOf],
  );

  const toggleRow = useCallback(
    (i: number) => {
      const el = audioRef.current;
      // The row that is already going: pause/resume rather than restart, so the position keeps.
      if (el && current === i) {
        if (el.paused) void el.play().catch(() => {});
        else el.pause();
        return;
      }
      startAt(i);
    },
    [current, startAt],
  );

  const playable = rows.filter((r) => fileOf(r)).length;

  // --- adding ----------------------------------------------------------------------

  // One act here — SEARCH, over the workspace's audio files; a new one arrives in the file tree.
  const [searching, setSearching] = useState(false);
  const [query, setQuery] = useState("");
  const queryRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (searching) queryRef.current?.focus();
  }, [searching]);

  /** The records a line may name, the vocabulary's own list, so the search and the completion agree. */
  const records = useMemo(
    () => playlistVocabulary(files).valuesAt?.([], "items") ?? [],
    [files],
  );
  const hits = useMemo(
    () => records.filter((r) => recordMatches(r, query)),
    [records, query],
  );

  const add = useCallback((record: string) => onAdd({ song: record }), [onAdd]);

  const cannotAdd =
    addRefusal || (files === null ? m["pl.filesNotLoaded"]() : "");

  // --- separating the whole set list --------------------------------------------------

  // The same work as `playlist stems`, driven from here so it can be WATCHED — and SEQUENTIAL, two at once on one machine finishing no sooner.
  const [separating, setSeparating] = useState(false);
  /** The chain THIS MACHINE separates with, or null when it has never been told — read and never asked for (`system stems`). */
  const [chain, setChain] = useState<string | null | undefined>(undefined);
  const [run, setRun] = useState<Run | null>(null);
  // Asked for when the bar opens rather than on mount, and re-asked on every opening: the settings panel is one click away.
  useEffect(() => {
    if (!separating) return;
    api
      .preferences()
      .then((p) => setChain(p.stemsChain))
      // A file service that will not answer is not a machine with no chain, but the button can do nothing either way.
      .catch(() => setChain(null));
  }, [separating]);

  /** `playlist stems`, run through the shell: the verb separates every record, skipping those already done. */
  const separateAll = useCallback(async () => {
    const total = rows.filter((r) => fileOf(r)).length;
    const say = (done: number, notes: string[], over: boolean) =>
      setRun({
        total,
        done,
        label: "",
        percent: null,
        skipped: 0,
        notes,
        over,
      });
    if (!total || !onRun) return say(0, [], true);
    say(0, [], false);
    // `onRun` does not reject: a refusal or a failure is told through its outcome.
    const outcome = { ok: false };
    await onRun(`playlist stems ${playlist}`, outcome);
    if (outcome.ok) say(total, [], true);
    else say(0, [m["pl.stemsRefused"]()], true);
  }, [rows, fileOf, onRun, playlist]);

  return (
    <section className="flex h-full min-h-0 flex-col">
      {/* Off screen and never controlled by React: a `<audio>` re-rendered with a new src prop restarts what is playing. */}
      <audio
        ref={audioRef}
        className="hidden"
        onPlay={() => setPlaying(true)}
        onPause={() => setPlaying(false)}
        // The next line begins where this one ended; only ours can end, this element being this pane's alone.
        onEnded={() => current !== null && startAt(current + 1)}
        onError={() => setNotice(m["pl.wontPlay"]())}
      >
        <track kind="captions" />
      </audio>

      <header className="flex shrink-0 flex-wrap items-center gap-2 pb-2">
        {/* The night-before job, here rather than on the desk because it is about the SET LIST and not about one record. */}
        <Button
          size="sm"
          variant={separating ? "default" : "secondary"}
          disabled={!playable}
          title={
            playable ? m["pl.separateHint"]() : m["pl.nothingToSeparate"]()
          }
          onClick={() => setSeparating((s) => !s)}
        >
          <Split className="mr-1 h-4 w-4" /> {m["pl.separate"]()}
        </Button>
        <span className="type-status text-fg-3">
          {m["pl.rowCount"]({
            n: rows.length,
            s: rows.length > 1 ? "s" : "",
          })}
          {playable === rows.length
            ? ""
            : ` · ${m["pl.playableCount"]({ n: playable })}`}
        </span>
      </header>

      {/* The separation bar, above the list like the search: what it is working through stays visible under it. */}
      {separating ? (
        <StemsBar
          chain={chain}
          run={run}
          onSettings={onRun && (() => void onRun("ui settings io"))}
          onStart={() => void separateAll()}
          startLabel={m["pl.start"]({ n: playable })}
          startDisabled={(!!run && !run.over) || !chain || !playable}
          onClose={() => setSeparating(false)}
        />
      ) : null}

      <div className="min-h-0 flex-1 overflow-auto">
        {rows.length === 0 ? (
          <div className="flex flex-col items-start gap-3 p-4">
            <p className="text-sm text-muted-foreground">
              {parts((h) => m["pl.emptyBody"](h), {
                items: <span className="font-mono">items:</span>,
              })}
            </p>
          </div>
        ) : (
          <RowGroup role="list">
            {rows.map((r, i) => {
              const problem = !r.path
                ? m["pl.notARecord"]()
                : !r.present && files !== null
                  ? m["pl.notInWorkspace"]()
                  : "";
              const missing = !problem && !r.present;
              const playingHere = current === i && playing;
              return (
                <Row
                  // The index IS the identity: a playlist may name the same song twice, and keying on the ref would make those one row.
                  // biome-ignore lint/suspicious/noArrayIndexKey: see above
                  key={i}
                  role="listitem"
                  // Draggable onto a DECK — the record's path travels, see apps/k7-daw-frontend/packages/k7-plugins/docs/turntables/setList.md.
                  draggable={!problem && !missing}
                  onDragStart={(e) => {
                    if (!r.path) return;
                    e.dataTransfer.setData(
                      RECORD_DRAG_TYPE,
                      packRecordDrag({ record: r.path }),
                    );
                    e.dataTransfer.effectAllowed = "copy";
                  }}
                  selected={current === i}
                  className={cn(
                    !problem &&
                      !missing &&
                      "cursor-grab active:cursor-grabbing",
                    // A file outside the workspace reads amber.
                    problem && "text-warn",
                  )}
                  icon={
                    playingHere ? (
                      <Pause />
                    ) : (
                      <span className="type-mono-value">{i + 1}</span>
                    )
                  }
                  detail={
                    problem ||
                    (r.path && recordFolder(r.path) ? recordFolder(r.path) : "")
                  }
                  trailing={
                    <span className="flex shrink-0 items-center">
                      {/* Already separated, so the night-before run will skip it: its four stems are beside it. */}
                      {r.stems ? (
                        <span
                          className="px-1 text-fg-3"
                          title={m["pl.alreadySplit"]()}
                        >
                          <Split className="size-4" />
                        </span>
                      ) : null}
                      <Button
                        size="icon-sm"
                        variant="ghost"
                        disabled={!!problem || missing}
                        title={problem || undefined}
                        aria-label={
                          playingHere ? m["pl.pauseRow"]() : m["pl.playRow"]()
                        }
                        onClick={() => toggleRow(i)}
                      >
                        {playingHere ? (
                          <Pause className="size-4" />
                        ) : (
                          <Play className="size-4" />
                        )}
                      </Button>
                      {/* The running order and what is on it: the two acts that would otherwise mean opening the file. */}
                      {onMove ? (
                        <>
                          <Button
                            size="icon-sm"
                            variant="ghost"
                            disabled={i === 0 || !!editRefusal}
                            title={editRefusal || m["pl.moveUp"]()}
                            aria-label={m["pl.moveUp"]()}
                            onClick={() => onMove(i, i - 1)}
                          >
                            <ChevronUp className="size-4" />
                          </Button>
                          <Button
                            size="icon-sm"
                            variant="ghost"
                            disabled={i === rows.length - 1 || !!editRefusal}
                            title={editRefusal || m["pl.moveDown"]()}
                            aria-label={m["pl.moveDown"]()}
                            onClick={() => onMove(i, i + 1)}
                          >
                            <ChevronDown className="size-4" />
                          </Button>
                        </>
                      ) : null}
                      {onRemove ? (
                        <Button
                          size="icon-sm"
                          variant="ghost"
                          disabled={!!editRefusal}
                          title={editRefusal || m["pl.removeRow"]()}
                          aria-label={m["pl.removeRow"]()}
                          onClick={() => {
                            // The row playing IS this one: silence it before the line under the transport goes.
                            if (current === i) {
                              audioRef.current?.pause();
                              setCurrent(null);
                            }
                            onRemove(i);
                          }}
                        >
                          <X className="size-4" />
                        </Button>
                      ) : null}
                    </span>
                  }
                >
                  {labelOf(r)}
                </Row>
              );
            })}
          </RowGroup>
        )}
      </div>

      {/* The search sits under the list, where a record is added: the list grows above it, which is the only confirmation there is. */}
      <div className="flex max-h-[55%] min-h-0 shrink-0 flex-col pt-2">
        {searching ? (
          <div className="min-h-0 flex-1 overflow-auto px-3 pb-3">
            {hits.length === 0 ? (
              <p className="py-3 text-fg-2 text-sm">{m["pl.noHit"]()}</p>
            ) : (
              <RowGroup>
                {hits.slice(0, MAX_HITS).map((r) => (
                  <RowButton
                    key={r}
                    icon={<Disc3 />}
                    detail={r}
                    trailing={<Plus className="size-4 text-fg-3" />}
                    onClick={() => add(r)}
                  >
                    {recordName(r)}
                  </RowButton>
                ))}
              </RowGroup>
            )}
            {/* Never a silent cap: a search that quietly hid the song you wanted is worse than one that says to type another word. */}
            {hits.length > MAX_HITS ? (
              <p className="type-status py-2 text-fg-3">
                {m["pl.moreHits"]({ n: hits.length - MAX_HITS })}
              </p>
            ) : null}
          </div>
        ) : null}
        <div className="flex shrink-0 items-center gap-2 px-3 py-2">
          <Search className="h-4 w-4 shrink-0 text-muted-foreground" />
          <Input
            ref={queryRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSearching(true);
            }}
            onFocus={() => setSearching(true)}
            onKeyDown={(e) => {
              // The shell binds bare keys; a search field must keep its own.
              e.stopPropagation();
              // Escape closes without adding; Enter takes the first hit, which is what 'type three letters and press return' expects.
              if (e.key === "Escape") return setSearching(false);
              if (e.key === "Enter" && hits[0]) add(hits[0]);
            }}
            placeholder={m["pl.searchPlaceholder"]()}
            aria-label={m["media.playlists.search"]()}
            disabled={!!cannotAdd}
            title={cannotAdd}
            className="min-w-0 flex-1"
          />
          <span className="type-mono-value shrink-0 text-fg-3">
            {hits.length}
          </span>
          {searching ? (
            <Button
              size="icon-sm"
              variant="ghost"
              aria-label={m["pl.closeSearch"]()}
              onClick={() => setSearching(false)}
            >
              <X className="h-3.5 w-3.5" />
            </Button>
          ) : null}
        </div>
      </div>

      <footer className="type-status flex shrink-0 items-center gap-2 pt-2 text-fg-3">
        <ListMusic className="h-3.5 w-3.5" />
        <span className="min-w-0 flex-1 truncate">
          {notice ||
            (current !== null && rows[current]
              ? m["pl.playing"]({
                  label: labelOf(rows[current] as ResolvedRecord),
                })
              : m["pl.footerHint"]())}
        </span>
      </footer>
    </section>
  );
}
