// MixView.tsx — the desk: two decks facing each other, each with its tempo on the outer edge
// and its channel strip on the inner one, and the crossfader in the gap between them. What
// plays comes from the playlist the desk's surface names, chosen at the record itself. See ../../docs/mix.md.
// A deck's column is `MixDeck.tsx` over `DeckSide.tsx`, its buttons `DeckButtons.tsx`; what is left is
// the state both columns share (waveforms, the browser, the set list).
// It DRAWS the decks and owns nothing about them: the document is `useDeckSet`, held by Studio,
// because this view is unmounted the moment somebody opens the graph and a set outlives that.

import { PLAYLISTS_DIR } from "@studio/k7-wasm/consts";
import { playlistPathOf } from "@studio/k7-wasm/schema";
import type { RecordDrag } from "@studio/song-schema/records";
import { Button } from "@studio/ui/button";
import { Card } from "@studio/ui/card";
import { AudioLines, Music, RefreshCw } from "@studio/ui/icons";
import { useCallback, useEffect, useRef, useState } from "react";
import { CodeText } from "../../CodeText";
import { m } from "../../paraglide/messages.js";
import { beatPhase } from "../beats";
import type { DeckId, DeckTempo } from "../deckVocabulary";
import { deskLayout, recordRow } from "../deskLayout";
import type { Deck } from "../engine";
import { loopSpan } from "../loop";
import { MIX_VIEW_FILE } from "../mixView";
import { NO_PREWARM, type Prewarm, prewarming, prewarmSet } from "../prewarm";
import { recordPath, service } from "../service";
import {
  entryForDrag,
  NO_PLAYLIST,
  resolveSet,
  type SetEntry,
  type SetList,
} from "../setList";
import { useContentHeight } from "../useContentHeight";
import type { DeckSet } from "../useDeckSet";
import { CueButton, FxButtons, SyncButton } from "./DeckButtons";
import type { Wave } from "./DeckSide";
import { InSightContext, useWatchSight } from "./inSight";
import { MixDeck } from "./MixDeck";
import { MasterMeter, MixerColumn } from "./MixerColumn";
import { SongBrowser } from "./SongBrowser";

/** The previous span while start and end are unchanged — identity is what an effect compares. */
function useSteadySpan(
  span: { start: number; end: number } | null,
): { start: number; end: number } | null {
  const kept = useRef(span);
  if (kept.current?.start !== span?.start || kept.current?.end !== span?.end)
    kept.current = span;
  return kept.current;
}

export function MixView({
  deck,
  set: decks,
  playlist,
  file,
}: {
  /** The engine link, held by the host so every view shares one. */
  deck: Deck;
  /** The playlist this surface declares, by name — for the messages that must say WHICH file is wrong. */
  playlist?: string;
  /** The surface's own file, so "add a `playlist:` line" can name where. */
  file: string;
  /** The decks as a document — everything this view shows and every gesture it forwards. */
  set: DeckSet;
}) {
  // Kept the same object while its edges hold: a fresh one per render would restart the waveform's loop.
  const spanA = useSteadySpan(
    loopSpan(decks.loopOf("deckA"), decks.tempos.deckA),
  );
  const spanB = useSteadySpan(
    loopSpan(decks.loopOf("deckB"), decks.tempos.deckB),
  );
  const [root, setRoot] = useState<string | null>(null);
  const [set, setSet] = useState<SetList | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Per deck. Three states rather than a nullable envelope: "computing" and "never" are two sentences.
  const [waves, setWaves] = useState<Partial<Record<DeckId, Wave>>>({});
  /** Which deck's song browser is open, if any — one dialog, told which deck it is for. */
  const [browsing, setBrowsing] = useState<DeckId | null>(null);
  // Which load each deck's waveform answers: a slow analysis of the OUTGOING record must not set the incoming one's tempo.
  const waveFor = useRef<Partial<Record<DeckId, number>>>({});
  // Whether the desk is on screen, for the meters and the records to go still while it is hidden (inSight.ts).
  const rootRef = useRef<HTMLDivElement>(null);
  const inSight = useWatchSight(rootRef);
  const refresh = useCallback(async () => {
    try {
      const [at, files] = await Promise.all([service.root(), service.files()]);
      setRoot(at);
      // TWO sources, and the PLAYLIST decides what is drawn — see ../../docs/setList.md.
      setSet(
        decks.playlistText === undefined
          ? NO_PLAYLIST
          : resolveSet(decks.playlistText, files),
      );
      setError(null);
    } catch (e) {
      setSet(null);
      setError(m["mix.noFileService"]() + ` (${(e as Error).message})`);
    }
  }, [decks.playlistText]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** Analyse the set list before anybody reaches for it — ../../docs/prewarm.md. */
  const [warm, setWarm] = useState<Prewarm>(NO_PREWARM);
  useEffect(() => {
    if (!set?.entries.length) return;
    // Every record the playlist NAMES and the workspace holds; the rest have nothing to analyse.
    const paths = set.entries
      .map((s) => s.file)
      .filter((p): p is string => !!p);
    let alive = true;
    void prewarmSet(paths, service.envelope, () => alive, setWarm);
    // A second playlist, or a closed desk: the walk stops between records rather than at the end.
    return () => {
      alive = false;
    };
  }, [set]);

  /** A line dropped from the playlist panel — resolved against THIS surface's set list, or refused by name. */
  const dropped = (deckId: DeckId, drag: RecordDrag) => {
    const row = entryForDrag(set?.entries ?? [], drag);
    // A panel may show another playlist than the one the surface names; saying so beats a dead drop.
    if (!row) return setError(m["mix.notInPlaylist"]({ song: drag.record }));
    load(deckId, row);
  };

  /** Put a record on a deck: it PLAYS at once, and the analysis lands separately, when it lands. */
  const load = (deckId: DeckId, s: SetEntry) => {
    const rel = s.file;
    if (!deck.engine || !root || !rel) return;
    setError(null);
    decks.putOn(deckId, {
      path: recordPath(root, rel),
      title: s.name,
      artist: s.artist,
      id: s.id,
      // Whether its four stems are beside it — it decides whether the stem lanes are filled.
      stems: s.stems,
    });

    setWaves((w) => ({ ...w, [deckId]: { k: "loading" } }));
    // A new record starts at ITS own tempo, whatever the outgoing one was doing.
    decks.clearTempo(deckId);
    const token = (waveFor.current[deckId] ?? 0) + 1;
    waveFor.current[deckId] = token;
    const current = () => waveFor.current[deckId] === token;
    void service
      .envelope(rel)
      .then((env) => {
        if (!current()) return;
        setWaves((w) => ({ ...w, [deckId]: { k: "ready", env } }));
        // The tempo arrives WITH the waveform — the sidecar carries both.
        const grid = beatPhase(env.transients, env.bpm);
        const t: DeckTempo = {
          natural: env.bpm,
          seconds: env.seconds,
          target: env.bpm,
          // Where the beat grid starts, carried ONLY when the onsets really said so — ../../docs/bpm.md.
          ...(grid.confident ? { phase: grid.phase } : {}),
        };
        decks.measured(deckId, t);
      })
      // No error card: the record plays regardless, but the REASON goes into the strip to be acted on.
      .catch((e: Error) => {
        if (current())
          setWaves((w) => ({ ...w, [deckId]: { k: "none", why: e.message } }));
      });
  };

  const { tempos, master, mix } = decks;

  const ready = deck.phase.k === "ready";

  /** What each deck draws, decided by the room the decks row has — measured, not told: ../../docs/deskLayout.md. */
  const deckRow = useRef<HTMLDivElement>(null);
  // The stem block only costs height when there is no record for it to sit beside — ../../docs/deskLayout.md.
  const stemLive = decks.stemLive("deckA") || decks.stemLive("deckB");
  // …and the echo costs the STRIP a row, on the same "either deck wires one" rule.
  const echoLive = decks.echoLive("deckA") || decks.echoLive("deckB");
  const layout = deskLayout(useContentHeight(deckRow), stemLive, echoLive);
  /** How tall a deck's TOP row is, which the crossfader restates to land on the cue row's line. */
  const crossfadeHead = recordRow(layout, stemLive);

  return (
    <InSightContext.Provider value={inSight}>
      <div ref={rootRef} className="flex min-h-0 flex-1 flex-col">
        {/* Anything wrong, across the top: there is no middle column to put it in. */}
        {error ||
        decks.deskError ||
        set?.issues.length ||
        decks.problems.length ? (
          <div className="flex shrink-0 flex-col gap-2 px-4 pt-4">
            {/* This view's own trouble and the DESK's, one card each: they are different things to go and fix. */}
            {error ? (
              <Card className="p-3 text-sm text-error">{error}</Card>
            ) : null}
            {decks.deskError ? (
              <Card className="p-3 text-sm text-error">
                {decks.deskError}
              </Card>
            ) : null}
            {/* A set list that will not read is the project's problem, so it is reported apart and names its file. */}
            {set?.issues.length ? (
              <Card className="p-3 text-sm text-error">
                <div className="font-semibold">
                  {playlist ? playlistPathOf(playlist) : PLAYLISTS_DIR}
                </div>
                {set.issues.map((i) => (
                  <div key={i} className="font-mono text-xs">
                    {i}
                  </div>
                ))}
              </Card>
            ) : null}
            {/* Not an error but a desk pointed at no set list, said with the one line that fixes it. */}
            {set?.absent ? (
              <Card className="p-3 text-sm text-muted-foreground">
                <CodeText text={m["mix.noPlaylist"]({ file })} />
              </Card>
            ) : null}
            {/* A binding naming something the project has not got, reported by CONTROL: that is what looks broken. */}
            {decks.problems.length ? (
              <Card className="p-3 text-sm text-error">
                <div className="font-semibold">{MIX_VIEW_FILE}</div>
                {decks.problems.map((p) => (
                  <div key={p.control} className="font-mono text-xs">
                    {p.control} : {p.message}
                  </div>
                ))}
              </Card>
            ) : null}
          </div>
        ) : null}

        {/* --- the two decks, facing each other -------------------------------------------- */}
        <div
          ref={deckRow}
          className="flex min-h-0 flex-1 gap-4 p-4 max-md:overflow-x-auto"
        >
          <MixDeck
            id="deckA"
            decks={decks}
            layout={layout}
            wave={waves.deckA}
            driven={master === "deckB"}
            canCorrect
            onCorrect={decks.correctTempo}
            onPick={() => setBrowsing("deckA")}
            onDropSong={(d) => dropped("deckA", d)}
            loopSpan={spanA}
          />

          {/* Between the two, where a hand crosses from one to the other — ../../docs/mix.md.
            The two CHANNEL faders live here too, over the crossfader, which is a mixing desk's
            own layout and what this column was empty for. */}
          <MixerColumn
            mix={mix}
            quantise={decks.quantise}
            onQuantise={decks.toggleQuantise}
            layout={layout}
            head={crossfadeHead}
            ready={ready}
            levels={{
              deckA: decks.levelOf("deckA") ?? 0,
              deckB: decks.levelOf("deckB") ?? 0,
            }}
            levelLive={{
              deckA: decks.levelLive("deckA"),
              deckB: decks.levelLive("deckB"),
            }}
            meterTracks={{
              deckA: decks.meterTrack("deckA"),
              deckB: decks.meterTrack("deckB"),
            }}
            trackOrder={decks.trackOrder}
            onLevel={(deckId, dB) => decks.turnLevel(deckId, dB)}
            onChange={decks.crossfade}
          />

          <MixDeck
            id="deckB"
            decks={decks}
            layout={layout}
            wave={waves.deckB}
            driven={master === "deckA"}
            canCorrect
            onCorrect={decks.correctTempo}
            onPick={() => setBrowsing("deckB")}
            onDropSong={(d) => dropped("deckB", d)}
            loopSpan={spanB}
          />
        </div>

        {/* One browser, told which deck it is for: two dialogs would be two states able to disagree. */}
        <SongBrowser
          side={browsing === "deckB" ? "B" : "A"}
          songs={set?.entries ?? []}
          open={browsing !== null}
          onOpenChange={(o) => setBrowsing(o ? browsing : null)}
          onPick={(song) => browsing && load(browsing, song)}
        />

        {/* --- the crossfader, across the bottom ----------------------------------------- */}
        <footer className="flex h-[10%] min-h-[4.5rem] shrink-0 items-center gap-6 border-t px-6">
          {/* At the two ENDS, flanking the crossfader: the button says who LEADS, not who follows. */}
          <SyncButton
            side="A"
            on={master === "deckA"}
            enabled={!!tempos.deckA?.natural && !!tempos.deckB?.natural}
            beats={decks.canAlign}
            onClick={() => decks.toggleSync("deckA")}
          />

          {/* Deck A's two MOMENTARY gestures, beside its sync — see ../../docs/fx.md. */}
          <FxButtons
            side="A"
            censorable={decks.canCensor("deckA")}
            onCensor={(on) => decks.censor("deckA", on)}
            echoLive={decks.echoLive("deckA")}
            echoingOut={decks.echoingOut("deckA")}
            onEchoOut={(on) => decks.echoOut("deckA", on)}
          />

          {/* What the crossfader used to fill: it lives BETWEEN the decks now, on the cue row's line. */}
          <div className="min-w-0 flex-1" />

          {/* The headphones, beside the metronome: both are about what YOU hear and not what the room does. */}
          <CueButton
            side="A"
            on={decks.cueOn("deckA")}
            wired={decks.cueWired("deckA")}
            live={ready && decks.cueLive("deckA")}
            onClick={() => decks.toggleCue("deckA")}
          />
          <CueButton
            side="B"
            on={decks.cueOn("deckB")}
            wired={decks.cueWired("deckB")}
            live={ready && decks.cueLive("deckB")}
            onClick={() => decks.toggleCue("deckB")}
          />

          {/* Next to the transport since it is about what you HEAR, and dark unless a single beat exists. */}
          <Button
            size="sm"
            variant={decks.clicking ? "default" : "secondary"}
            disabled={!ready || !decks.clickDeck}
            title={
              !ready
                ? m["mix.noEngine"]()
                : decks.clickDeck
                  ? m["mix.metronome"]({
                      side: decks.clickDeck === "deckA" ? "A" : "B",
                    })
                  : // The rule, said as the reason it is off rather than as a rule.
                    m["mix.metronomeOff"]()
            }
            onClick={decks.toggleMetronome}
            aria-pressed={decks.clicking}
          >
            <AudioLines className="h-4 w-4" />
          </Button>

          {/* Only while there is something left to analyse — ../../docs/prewarm.md. */}
          {prewarming(warm) ? (
            <span
              className="shrink-0 font-mono text-[10px] text-muted-foreground tabular-nums"
              title={m[warm.failed ? "mix.prewarmFailed" : "mix.prewarm"](warm)}
            >
              <Music className="size-3" /> {warm.done}/{warm.total}
            </span>
          ) : null}

          <Button
            size="sm"
            variant="ghost"
            className="shrink-0 text-xs"
            title={m["mix.refresh"]()}
            onClick={() => void refresh()}
          >
            <RefreshCw className="h-3.5 w-3.5" />
          </Button>

          {/* The BLEND, and the only meter here that reads it — a channel meter is pre-fader. */}
          <MasterMeter />

          <FxButtons
            side="B"
            censorable={decks.canCensor("deckB")}
            onCensor={(on) => decks.censor("deckB", on)}
            echoLive={decks.echoLive("deckB")}
            echoingOut={decks.echoingOut("deckB")}
            onEchoOut={(on) => decks.echoOut("deckB", on)}
          />

          <SyncButton
            side="B"
            on={master === "deckB"}
            enabled={!!tempos.deckA?.natural && !!tempos.deckB?.natural}
            beats={decks.canAlign}
            onClick={() => decks.toggleSync("deckB")}
          />
        </footer>
      </div>
    </InSightContext.Provider>
  );
}

/** THIS deck's transport, at the foot of its tempo column. Drawn GREYED with nothing on the deck. */
