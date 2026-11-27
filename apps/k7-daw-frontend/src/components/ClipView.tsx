// ClipView.tsx — a clip's CONTENT: the AudioClipPanel for audio, the piano roll for MIDI, and for
// a live sequence FILE the roll of the notes parsed out of its text. Each draws its own header and
// close button. Two hosts mount it — the Editor bottom panel, and the graph slot when a clip is
// clicked in the timeline (see docs/shell.md). There is ONE audio clip panel, and clicking a clip
// reaches it: `WaveformView` draws the same picture and can act on none of it.

import { DEFAULT_METRONOME_LEVEL } from "@studio/engine-protocol/build";
import { fileStem } from "@studio/k7-wasm/paths";
import { seqWindowFromText, windowSpan } from "@studio/k7-wasm/rules";
import { readSequenceNotes } from "@studio/k7-wasm/text";
import { beatsToPosition } from "@studio/k7-wasm/timecode";
import { DockStatus } from "@studio/ui/editor-dock";
import { type ComponentProps, type RefObject, useMemo } from "react";
import type { MenuCtx } from "../contextMenu";
import type { ClipGridHandle, OnGridSelection } from "../gridCaret";
import { m } from "../paraglide/messages.js";
import { usePaneGraph } from "../queries/graph";
import { usePaneProjectDoc } from "../queries/projectDoc";
import { rackFor, rackKeyNames } from "../rackFor";
import type { RollMode } from "../rollMode";
import { trackPlacing } from "../rows";
import { useReloadKey, useVerbs } from "../state/shellStore";
import { eventIdOf, takesEventOf } from "../takes";
import type { WebClip, WebNote, WebProject } from "../types";
import type { WaveMode } from "../waveMode";
import { type AudioClipHandle, AudioClipPanel } from "./AudioClipPanel";
import { GraphSlotPane } from "./GraphSlotPane";
import { PianoRoll, type RollHandle } from "./PianoRoll";
import type { PanelTakes, TakesHost } from "./TakesView";

// What to show: a concrete clip (timeline caret or click), or the live sequence file open in the editor, parsed here.
export type EditTarget =
  // `track` when the opener knows the lane: a duplicated lane holds clips with the SAME ids as its original.
  | { kind: "clip"; project: WebProject; clip: WebClip; track?: string }
  | { kind: "seq"; path: string; content: string };

// Enough of a project for the roll (tempo + time signature) until the real doc loads, so a sequence still previews.
const FALLBACK_PROJECT: WebProject = {
  name: "",
  tempo: 120,
  timeSigNumerator: 4,
  timeSigDenominator: 4,
  masterVolume: 0, // unity — a stand-in draws no fader, and a roll asks nothing of it
  tracks: [],
  groups: [],
  metronome: false,
  metronomeLevel: DEFAULT_METRONOME_LEVEL,
  countIn: 0,
  markers: [],
};

// The live sequence file's notes as a roll, re-parsed as the text changes — parsing is local and cheap.
function SeqRoll({
  path,
  content,
  onCreateNote,
  onSetVelocity,
  onSetNoteRange,
  onMoveNote,
  onMoveNotes,
  onContextMenu,
  onRemoveNote,
  roll,
}: {
  path: string;
  content: string;
  onCreateNote?: (
    seqId: string,
    key: number,
    t0: number,
    t1: number,
  ) => void | Promise<void>;
  onSetVelocity?: (
    seqId: string,
    key: number,
    startBeat: number,
    vel: number,
  ) => void | Promise<void>;
  /** Move or stretch a note (`sequence note timerange`): where it starts NOW, then where it should start and end. */
  onSetNoteRange?: (
    seqId: string,
    key: number,
    startBeat: number,
    newStart: number,
    newEnd: number,
  ) => void | Promise<void>;
  /** Move a note to another PITCH (`sequence note move`), which keeps its duration. */
  onMoveNote?: (
    seqId: string,
    key: number,
    startBeat: number,
    newKey: number,
    newStart: number,
  ) => void | Promise<void>;
  /** Move SEVERAL notes in one write (`sequence notes move`): each [key, start, newKey, newStart] — the roll's dragged selection. */
  onMoveNotes?: (
    seqId: string,
    moves: [number, number, number, number][],
  ) => void | Promise<void>;
  onContextMenu?: (ctx: MenuCtx, x: number, y: number) => void;
  /** Remove one note (`sequence note remove`) — the roll's `remove` mode and its context menu. */
  onRemoveNote?: (
    seqId: string,
    key: number,
    startBeat: number,
  ) => void | Promise<void>;
  /** The caret and the mode, wired as the clip's roll wires them — what `x`, `ui roll <mode>` and the mode strip reach. */
  roll?: Pick<
    ComponentProps<typeof PianoRoll>,
    | "caretRef"
    | "active"
    | "setStatus"
    | "onSelection"
    | "onCommand"
    | "modeRef"
    | "onModeChange"
  >;
}) {
  // The shared document, for the tempo and the signature the roll is drawn in (queries/projectDoc.ts).
  const { doc: project } = usePaneProjectDoc(useReloadKey());

  const notes = useMemo<WebNote[]>(() => {
    return (readSequenceNotes(content)?.notes ?? []).map((n) => ({
      time: n.t,
      duration: n.d,
      key: n.k,
      // Only what the file states, as the compiled path does: absent IS the default 1.
      ...(n.v === undefined ? {} : { vel: n.v }),
    }));
  }, [content]);

  const name = fileStem(path) || "sequence";
  /** The file's own `from`/`to` as the roll's loop pair — for MIDI that IS the window (clipLoop.ts); the default `to` rounds UP TO THE BAR. */
  const loop = useMemo(() => {
    const num = project?.timeSigNumerator ?? 4;
    const den = project?.timeSigDenominator ?? 4;
    const win = seqWindowFromText(content, num, den);
    if (!win) return null;
    const bar = (num * 4) / den;
    const extent =
      Math.ceil(
        notes.reduce((m, n) => Math.max(m, n.time + n.duration), 0) / bar,
      ) * bar;
    return windowSpan(win, extent);
  }, [content, notes, project]);
  const clip: WebClip = {
    id: path,
    type: "midi",
    name,
    start: 0,
    length: 0,
    ...(loop ? { loopStart: loop.from, loopEnd: loop.to } : {}),
    notes,
  };

  return (
    <div className="h-full min-h-0">
      <PianoRoll
        project={project ?? FALLBACK_PROJECT}
        clip={clip}
        onCreateNote={
          onCreateNote
            ? (k, t0, t1) => onCreateNote(name, k, t0, t1)
            : undefined
        }
        onSetVelocity={
          onSetVelocity ? (k, t, v) => onSetVelocity(name, k, t, v) : undefined
        }
        onSetNoteRange={
          onSetNoteRange
            ? (k, t, ns, ne) => onSetNoteRange(name, k, t, ns, ne)
            : undefined
        }
        onMoveNote={
          onMoveNote
            ? (k, t, nk, ns) => onMoveNote(name, k, t, nk, ns)
            : undefined
        }
        onMoveNotes={
          onMoveNotes ? (moves) => onMoveNotes(name, moves) : undefined
        }
        onContextMenu={onContextMenu}
        onRemoveNote={
          onRemoveNote ? (k, t) => onRemoveNote(name, k, t) : undefined
        }
        {...roll}
      />
    </div>
  );
}

/** The roll's note edits as command lines over one runner, each returning its promise. */
function noteVerbs(run: (line: string) => Promise<void>) {
  return {
    onCreateNote: (seqId: string, key: number, t0: number, t1: number) =>
      run(`sequence note new ${seqId} ${key} ${t0} ${t1}`),
    // The velocity drag, committed like the timeline's clip drag: one command on release.
    onSetVelocity: (seqId: string, key: number, start: number, vel: number) =>
      run(`sequence note velocity ${seqId} ${key} ${start} ${vel}`),
    // Move and stretch are one verb, being one gesture with two grips.
    onSetNoteRange: (
      seqId: string,
      key: number,
      start: number,
      ns: number,
      ne: number,
    ) => run(`sequence note timerange ${seqId} ${key} ${start} ${ns} ${ne}`),
    // A drag that changed the PITCH keeps the duration and refuses an occupied slot.
    onMoveNote: (
      seqId: string,
      key: number,
      start: number,
      nk: number,
      ns: number,
    ) => run(`sequence note move ${seqId} ${key} ${start} ${nk} ${ns}`),
    // A dragged SELECTION: every note in one write and one Ctrl+Z.
    onMoveNotes: (seqId: string, moves: [number, number, number, number][]) =>
      run(`sequence notes move ${seqId} ${JSON.stringify(moves)}`),
    // The roll's `remove` mode, which is the caret's `x` reached by the pointer instead.
    onRemoveNote: (seqId: string, key: number, start: number) =>
      run(`sequence note remove ${seqId} ${key} ${start}`),
  };
}

export function ClipView({
  target,
  onClose,
  caretRef,
  onSelection,
  onPreviewNote,
  waveRef,
  onWaveView,
  rollRef,
  onRollMode,
  takesHost,
}: {
  target: EditTarget;
  onClose: () => void;
  /** Where the shell reaches this pane's CARET — forwarded to the roll. */
  caretRef?: RefObject<ClipGridHandle | null>;
  /** Whether the drawn grid has a rectangle — what makes ⇧L conditional (see OnGridSelection). */
  onSelection?: OnGridSelection;
  /** Hear one note through a TRACK's chain — over the engine's socket, not through a command; the track is found here. */
  onPreviewNote?: (track: string, key: number, vel: number) => void;
  /** The AUDIO leaf's mode, the counterpart of `caretRef` for the three grids — `ui wave <mode>` arrives through it. */
  waveRef?: RefObject<AudioClipHandle | null>;
  onWaveView?: (v: WaveMode | null) => void;
  /** …and the ROLL's, the third of the three — `ui roll <mode>`. */
  rollRef?: RefObject<RollHandle | null>;
  onRollMode?: (m: RollMode | null) => void;
  /** The takes mode's shell half — absent, neither panel offers the mode a record button (song docs/takes.md). */
  takesHost?: TakesHost;
}) {
  // Bumped by the shell after every command's reload — what keeps the open clip FRESH below.
  const reloadKey = useReloadKey();
  const {
    seek: onSeek,
    runCommand: command,
    openContextMenu: onContextMenu,
    setStatus,
    setLoop: onSetLoop,
  } = useVerbs();
  // The roll's gestures, one `sequence note` line each, ANSWERING the promise: the ghost is held until the reload lands (rollPending.ts).
  const notes = useMemo(() => noteVerbs(command), [command]);
  const {
    onCreateNote,
    onSetVelocity,
    onSetNoteRange,
    onMoveNote,
    onMoveNotes,
    onRemoveNote,
  } = notes;
  /** The open clip, kept FRESH: `target` is a snapshot from the click, and the placement is found again by (id, start). */
  const { doc: sharedProject } = usePaneProjectDoc(reloadKey);
  const freshProject = target.kind === "seq" ? null : sharedProject;
  const clip = useMemo(() => {
    if (target.kind === "seq") return null;
    const base = target.clip;
    for (const t of freshProject?.tracks ?? [])
      for (const c of [...(t.clips ?? []), ...(t.frozenMaterial ?? [])])
        if (c.id === base.id && Math.abs(c.start - base.start) < 1e-6) return c;
    return base;
  }, [target, freshProject]);
  const project =
    target.kind === "seq" ? null : (freshProject ?? target.project);
  // The EVENT the open clip belongs to, by its own id: a re-comp can replace the very piece that was clicked.
  const takes = useMemo((): PanelTakes | undefined => {
    if (target.kind === "seq" || !project || !takesHost) return undefined;
    const track = target.track || trackPlacing(target.project, target.clip.id);
    const ev = track
      ? takesEventOf(project, track, eventIdOf(target.clip))
      : null;
    return ev
      ? {
          host: takesHost,
          ev,
          num: project.timeSigNumerator,
          den: project.timeSigDenominator,
        }
      : undefined;
  }, [target, project, takesHost]);
  const seqId = clip ? clip.name || clip.id : "";
  /** The lane a MIDI clip plays on: the graph is read from it, for the sound names a rack gives the roll's keys. */
  const midiTrack =
    target.kind !== "seq" && target.clip.type === "midi"
      ? target.track || trackPlacing(target.project, target.clip.id)
      : "";
  /** The event as the `event …` verbs address it — lane and start — or null for a roll that shows no placement. */
  const eventAddr =
    clip && midiTrack && clip.length > 0 && target.kind !== "seq"
      ? `${midiTrack} ${beatsToPosition(
          clip.start,
          (project ?? target.project).timeSigNumerator,
          (project ?? target.project).timeSigDenominator,
        )}`
      : null;
  // The shared graph, asked only for a MIDI lane: undefined while the first read is in flight, null with none to read.
  const graphRead = usePaneGraph(reloadKey, !!midiTrack);
  const graph = !midiTrack
    ? null
    : (graphRead.graph ?? (graphRead.loading ? undefined : null));
  const rack = midiTrack ? rackFor(graph ?? null, midiTrack) : null;
  // The roll names a rack's keys after their samples; a lane with no rack keeps note names alone.
  const keyNames = useMemo(
    () => (rack ? rackKeyNames(rack) : undefined),
    [rack],
  );
  // No title bar: each leaf draws its own header, and the frame contributes the shell and one floating ✕.
  return (
    <GraphSlotPane>
      {target.kind === "seq" ? (
        <SeqRoll
          path={target.path}
          content={target.content}
          onCreateNote={onCreateNote}
          onSetVelocity={onSetVelocity}
          onSetNoteRange={onSetNoteRange}
          onMoveNote={onMoveNote}
          onMoveNotes={onMoveNotes}
          onContextMenu={onContextMenu}
          onRemoveNote={onRemoveNote}
          // No placement, so no seek and no event loop: the rest is the clip roll's wiring below.
          roll={{
            caretRef,
            active: true,
            setStatus,
            onSelection,
            onCommand: command,
            modeRef: rollRef,
            onModeChange: onRollMode,
          }}
        />
      ) : clip?.type === "midi" ? (
        graph === undefined ? (
          <DockStatus>{m["clipv.readingGraph"]()}</DockStatus>
        ) : (
          <div className="h-full min-h-0 min-w-0">
            <PianoRoll
              project={project ?? target.project}
              clip={clip}
              keyNames={keyNames}
              caretRef={caretRef}
              active
              setStatus={setStatus}
              onSelection={onSelection}
              onSetLoop={onSetLoop}
              // The EVENT's own loop, which only a placement has: the switch, and its two points in the notes' beats.
              onLoop={
                eventAddr
                  ? (on) =>
                      void command(
                        `event loop ${eventAddr} ${on ? "on" : "off"}`,
                      )
                  : undefined
              }
              onLoopPoints={
                eventAddr
                  ? (f, t) =>
                      void command(
                        `event loop points ${eventAddr} ${f.toFixed(4)} ${t.toFixed(4)}`,
                      )
                  : undefined
              }
              onSeek={onSeek}
              onCreateNote={
                onCreateNote
                  ? (k, t0, t1) => onCreateNote(seqId, k, t0, t1)
                  : undefined
              }
              onSetVelocity={
                onSetVelocity
                  ? (k, t, v) => onSetVelocity(seqId, k, t, v)
                  : undefined
              }
              onSetNoteRange={
                onSetNoteRange
                  ? (k, t, ns, ne) => onSetNoteRange(seqId, k, t, ns, ne)
                  : undefined
              }
              onMoveNote={
                onMoveNote
                  ? (k, t, nk, ns) => onMoveNote(seqId, k, t, nk, ns)
                  : undefined
              }
              onMoveNotes={
                onMoveNotes ? (moves) => onMoveNotes(seqId, moves) : undefined
              }
              onContextMenu={onContextMenu}
              // A sequence FILE has no placement, so no chain to hear it through: the roll is then silent, and honestly so.
              onPreviewNote={
                onPreviewNote && midiTrack
                  ? (k, v) => onPreviewNote(midiTrack, k, v)
                  : undefined
              }
              onRemoveNote={
                onRemoveNote ? (k, t) => onRemoveNote(seqId, k, t) : undefined
              }
              // The roll's mode: down from the shell so a key reaches it, up so a condition and the badge can read it.
              onCommand={command}
              modeRef={rollRef}
              onModeChange={onRollMode}
              takes={takes}
              reloadKey={reloadKey}
            />
          </div>
        )
      ) : (
        // An audio event opens the CLIP PANEL — the same one a take lands in, with its onsets, its trim and its normalise.
        clip && (
          <AudioClipPanel
            clip={clip.id}
            track={
              (target.kind === "clip" && target.clip.id === clip.id
                ? target.track
                : undefined) || trackPlacing(project ?? target.project, clip.id)
            }
            // The project's own, so the twelve-bar window is twelve of THIS song's bars.
            tempo={(project ?? target.project).tempo}
            beatsPerBar={(project ?? target.project).timeSigNumerator}
            // The clicked EVENT's beat — what puts the song's ruler over the wave.
            placedAt={clip.start}
            // …and the same beat as a TIMECODE, spelled HERE because it needs the project's time signature, denominator included.
            eventAt={beatsToPosition(
              clip.start,
              (project ?? target.project).timeSigNumerator,
              (project ?? target.project).timeSigDenominator,
            )}
            // …and the slice of the file it plays, already the intersection the compiler made (nodeProject.ts).
            eventWindow={{ from: clip.sourceFrom, to: clip.sourceTo }}
            // …and how many times it plays it: more than one is a loop, which the panel's switch and ruler edit.
            loops={clip.event?.loop ?? 1}
            // …and its own trim, which only an audio event has.
            gainDb={clip.event?.gain ?? 0}
            onClose={onClose}
            // The waveform's mode: down from the shell so a key reaches it, up so a condition and the badge can read it.
            viewRef={waveRef}
            onViewChange={onWaveView}
            takes={takes}
          />
        )
      )}
    </GraphSlotPane>
  );
}
