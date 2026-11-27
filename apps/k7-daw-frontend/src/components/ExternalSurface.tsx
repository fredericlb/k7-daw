// ExternalSurface.tsx — ONE surface alone, at `<project>/@external/<name>`: what a tablet by the
// decks opens, driving the same engine as the laptop behind it.
//
// Not the shell with things hidden. It mounts the engine link and hands it to whichever of the
// four surface components the declaration names — what the shell adds around a surface is a
// LAYOUT, and none of that is what a surface is made of.
//
// What it DOES add is a footer: one command line and the way back, and the transport, at the head on its own
// machine and at the foot, in the bar's capsule, on another device's surface (`lan`). See ../docs/external.md.

import type { EngineClient, EngineUrl } from "@studio/engine-browser";
import { PluginParamsStore } from "@studio/engine-browser/params";
import {
  EngineStore,
  EngineStoreProvider,
  useStoreState,
} from "@studio/engine-browser/store";
import type { TransportAction } from "@studio/engine-protocol";
import type { Deck } from "@studio/k7-plugins/turntables/engine";
import type { ProjectSurface } from "@studio/k7-wasm/types";
import { surfacesUrl } from "@studio/project-shell/launch";
import { Button } from "@studio/ui/button";
import { ChevronLeft, House, Unplug } from "@studio/ui/icons";
import { Spinner } from "@studio/ui/spinner";
import type { WebVideoProject } from "@studio/web-protocol";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, engines } from "../api";
import type { CommandPrefs } from "../commandPrefs";
import type { TimelineHandle } from "../components/TimelineTab";
import { useCommandKeys } from "../hooks/useCommandKeys";
import { useEngineLink } from "../hooks/useEngineLink";
import { useFullscreen } from "../hooks/useFullscreen";
import { useProjectDoc } from "../hooks/useProjectDoc";
import { useRevision } from "../hooks/useRevision";
import { useScreenPresence } from "../hooks/useScreenPresence";
import { useStayAwake } from "../hooks/useStayAwake";
import {
  type SurfaceCommands,
  useSurfaceCommands,
} from "../hooks/useSurfaceCommands";
import { useSurfaces } from "../hooks/useSurfaces";
import { useTurntableDeck } from "../hooks/useTurntableDeck";
import { useVideoDoc } from "../hooks/useVideoDoc";
import { parts } from "../i18nParts";
import { type ConditionCtx, effectiveKeys } from "../keys";
import { BARE_TRANSPORT, isUiLine } from "../localCommands";
import { m } from "../paraglide/messages.js";
import { DEVICE_SCOPE } from "../queries/client";
import { FloatingBar } from "../shell/FloatingBar";
import { MiniTransport } from "../shellMeta";
import { deviceSurfaceDrawer } from "../surfacePanes";
import type { CommandInfo } from "../types";
import { useShellLayout } from "../useIsMobile";
import { ExternalFooter } from "./ExternalFooter";

/** No caret and no rectangle until a surface says otherwise — a frozen identity, the key listener taking it as a dependency. */
const NO_CONDITIONS: ConditionCtx = {};

/** The `ui` verbs that mean nothing on a one-surface screen and are not worth saying so about — what Esc and F6 fall through to. */
const QUIET_UI = new Set([
  "close",
  "global",
  "focus next",
  "focus prev",
  "popin close",
  "slot close",
]);

const noPlace = { current: () => false };

export function ExternalSurface({
  name,
  project,
  engineUrl,
  userEmail,
  lan,
}: {
  /** The surface's id, off the address — `@external/<name>`. */
  name: string;
  /** Workspace-relative, as every file route here takes it. */
  project: string;
  engineUrl: EngineUrl;
  userEmail: string;
  /** Another device's surface: which device it is, and the name this one tells it. Absent on a surface opened on its own machine. */
  lan?: { engineId: string; device: string };
}) {
  const store = useRef(new EngineStore()).current;
  const layout = useShellLayout();
  // A remote tells the device whose surface it shows, so that device cannot take the same pane (surfaces/presence.ts).
  useScreenPresence(lan?.engineId ?? "", name, lan?.device ?? "");
  const [clockSeconds, setClockSeconds] = useState(false);
  const pluginParamsStore = useRef(new PluginParamsStore()).current;
  // A surface is watched and not touched, and this device may be one no lock is offered on.
  useStayAwake(true);
  const fullscreen = useFullscreen();
  // The one live slice this level draws; NOT the playhead, whose clock subscribes for itself (shellMeta.tsx).
  const playing = useStoreState(store, (s) => s.playing);
  /** Where a fault goes on a screen with no status line — one line in the footer, which is what a phone has room for. */
  const [status, setStatus] = useState("");
  const pushLog = useCallback(() => {}, []);

  const {
    engineRef,
    connected,
    engineReady,
    engineBusyRef,
    transport,
    seek,
    setPluginParam,
    watchPluginParams,
  } = useEngineLink(
    engineUrl,
    userEmail,
    store,
    pluginParamsStore,
    setStatus,
    pushLog,
    // A bare surface holds no project to mark: every warning toasts.
    noPlace,
  );

  const rev = useRevision();

  const deck = useTurntableDeck({
    store,
    engineRef,
    engineReady,
    watchPluginParams,
    pluginParamsStore,
    engineUrl,
  });

  /** The caret, the zoom and the selection, reported UP by the surface that has them — the conditional bindings read them (keys.ts). */
  const [conditions, setConditions] = useState<ConditionCtx>(NO_CONDITIONS);
  /** The arrangement's handle, when one is mounted — how the caret verbs are answered locally. */
  const timelineRef = useRef<TimelineHandle>(null);
  /** Where the current roll began, in seconds — what ⇧Space goes back to. */
  const replayAt = useRef(0);

  /** The lines this screen answers itself: the bare transport verbs down the socket, the caret verbs to the surface, the rest refused in words. */
  const runLocal = useCallback(
    (line: string): boolean => {
      if (line === "transport replay") {
        // Back to where playback last started, then the bare verb: letting anything re-choose the position would move the replay.
        seek(replayAt.current);
        transport("play");
        return true;
      }
      if (BARE_TRANSPORT.includes(line)) {
        // …falling through to the slow path in the two cases the socket cannot answer for — dropping is right for a disabled button.
        if (!connected || engineBusyRef.current) return false;
        // Where this roll begins, for `replay` above, read before the verb goes; a stop or a rewind is not a start.
        if (line === "transport play" || line === "transport toggle")
          if (!store.getSnapshot().playing)
            replayAt.current = store.getSnapshot().playhead;
        transport(line.slice("transport ".length) as TransportAction);
        return true;
      }
      /** The CARET verbs — the mounted surface's own, through its handle; a surface with no arrangement says so rather than going quiet. */
      const ui = line.startsWith("ui ") ? line.slice(3).trim() : null;
      const onTimeline: Record<string, (h: TimelineHandle) => void> = {
        "transport locate": (h) => h.seekToCaret(),
        "transport loop": (h) => h.toggleLoop(),
        "ui select": (h) => h.toggleSelect(),
        "ui zoom": (h) => h.zoomIn(),
        "ui zoom out": (h) => h.zoomOut(),
        "ui zoom reset": (h) => h.zoomReset(),
        "ui ruler": (h) => h.toggleRuler(),
        "ui lanes": (h) => h.toggleLanes(),
        "ui automation": (h) => h.toggleAutomation(),
      };
      const act = onTimeline[line];
      if (act) {
        const h = timelineRef.current;
        if (h) act(h);
        else setStatus(m["extsrf.noArrangement"]({ line }));
        return true;
      }
      if (ui === null) return false;
      // The LAYOUT verbs go quiet rather than complaining: they are what Esc and Tab resolve to once nothing is open.
      if (QUIET_UI.has(ui)) return true;
      if (isUiLine(ui)) {
        setStatus(m["extsrf.editorOnly"]({ ui }));
        return true;
      }
      return false;
    },
    [connected, engineBusyRef, transport, seek, store],
  );

  /** Running a command, and the reload key every surface here reads — held at THIS level, a second copy being two ages of one project. */
  const commands = useSurfaceCommands(setStatus, runLocal);

  /** The command catalogue and this person's layer over it, fetched once: a remapped play key has to keep working on the tablet. */
  const [backendCmds, setBackendCmds] = useState<CommandInfo[]>([]);
  const [commandPrefs, setCommandPrefs] = useState<CommandPrefs>({});
  useEffect(() => {
    void api
      .commands()
      .then(setBackendCmds)
      .catch(() => {});
    void engines
      .prefs()
      .then((p: unknown) =>
        setCommandPrefs((p as { commands?: CommandPrefs })?.commands ?? {}),
      )
      .catch(() => {}); // never saved any: the defaults are the catalogue's own
  }, []);
  const effKeys = useMemo(() => effectiveKeys(commandPrefs), [commandPrefs]);

  /** The line a menu entry PRE-WRITES: ONE footer input, there being no command bar here and no dropping the entries that owe an argument. */
  const [line, setLine] = useState("");
  const lineRef = useRef<HTMLInputElement>(null);
  const prewrite = useCallback((command: string) => {
    setLine(command);
    lineRef.current?.focus();
  }, []);
  const focusLine = useCallback(() => lineRef.current?.focus(), []);

  // The same listener the shell runs, on the same keymap — hooks/useCommandKeys.ts.
  useCommandKeys({
    effKeys,
    conditions,
    runCommand: commands.runCommand,
    onDoubleShift: focusLine,
  });

  // The song's picture, for the frame — the same call the shell makes, on the key above rather than on the shell's.
  const { doc: videoDoc, error: videoError } = useVideoDoc(commands.reloadKey);
  // …and the tempo the transport clock counts in: the only two fields of the compiled document this level needs.
  const noMidiSeqs = useCallback(() => {}, []);
  const { recInfo } = useProjectDoc(DEVICE_SCOPE, commands.reloadKey, {
    onMidiSeqs: noMidiSeqs,
  });

  // The project's own list, on the same key as everything else here, which is driven by `/revision`.
  const { surfaces } = useSurfaces(commands.reloadKey);
  const surface = useMemo(
    () => surfaces.find((s) => s.id === name) ?? null,
    [surfaces, name],
  );

  // The list arrives a moment after this mounts, so 'not found' has to wait for it.
  if (!surfaces.length && !surface)
    return (
      <Centre>
        <Spinner inherit />
      </Centre>
    );

  if (!surface)
    return (
      <Centre>
        <Unplug className="h-8 w-8" aria-hidden="true" />
        <p className="max-w-sm text-center text-sm">
          {m["extsrf.noSurface"]({ name })}
        </p>
        <p className="max-w-sm text-center text-xs opacity-70">
          {parts((h) => m["extsrf.surfacesAre"](h), {
            dir: <code>surfaces/</code>,
          })}
        </p>
        <Back />
      </Centre>
    );

  const footer = (
    <ExternalFooter
      line={line}
      onLine={setLine}
      onRun={(l) => void commands.runCommand(l)}
      lineRef={lineRef}
      status={status}
      fullscreen={fullscreen}
      // The way back, which is not optional: installed as a PWA this screen has no address bar. A remote has the round Home button instead.
      back={lan ? null : <Back />}
      className={lan ? "mb-[4.5rem] shrink-0" : "shrink-0"}
    />
  );
  return (
    <EngineStoreProvider value={store}>
      <div className="relative flex h-full min-h-0 flex-col bg-ground text-fg-1">
        {lan ? (
          // The one way out of a surface that cannot change: a hard navigation, since this app's router ends at the project.
          <Button
            asChild
            variant="glass"
            size="round"
            className="absolute top-3 left-3 z-(--k7-z-header)"
          >
            <a
              href={surfacesUrl(window.location.origin, "k7")}
              aria-label={m["extsrf.home"]()}
              title={m["extsrf.home"]()}
            >
              <House className="size-5" />
            </a>
          </Button>
        ) : (
          /* Play/pause and the clock — the shell's `MiniTransport`; stop, rewind, record and the metronome stay out (../docs/external.md). */
          <MiniTransport
            playing={playing}
            waiting={!engineReady}
            tempo={recInfo.tempo}
            beatsPerBar={recInfo.beatsPerBar}
            onToggle={() => void commands.runCommand("transport toggle")}
          />
        )}
        <div className="flex min-h-0 flex-1 flex-col">
          <Drawn
            surface={surface}
            project={project}
            rev={rev}
            store={store}
            engineRef={engineRef}
            seek={seek}
            commands={commands}
            setStatus={setStatus}
            deck={deck}
            paramsStore={pluginParamsStore}
            setPluginParam={setPluginParam}
            watchPluginParams={watchPluginParams}
            videoDoc={videoDoc}
            videoError={videoError}
            prefs={commandPrefs}
            catalogue={backendCmds}
            prewrite={prewrite}
            onConditions={setConditions}
            timelineRef={timelineRef}
          />
        </div>
        {footer}
        {lan && (
          <FloatingBar
            surfaceOnly
            layout={layout}
            onCommandBar={noop}
            transport={{
              playing,
              transportWaiting: !engineReady,
              recordWithoutArm: false,
              transport,
              noArrangement: true,
              recordEnabled: false,
              midiArmed: null,
              openMidiInputs: () => Promise.resolve(false),
              setStatus: (t) => setStatus(t),
              engineRef,
            }}
            output={null}
            recInfo={recInfo}
            clockSeconds={clockSeconds}
            onClock={() => setClockSeconds((v) => !v)}
            map={null}
            meter={null}
            metronome={null}
            runCommand={(l) => commands.runCommand(l)}
          />
        )}
      </div>
    </EngineStoreProvider>
  );
}

const noop = () => {};

/** Which component draws this type — `SurfaceTab`'s switch inverted: out here there is no shell, so each is given what it actually needs. */
function Drawn({
  surface,
  project,
  rev,
  store,
  engineRef,
  seek,
  commands,
  setStatus,
  deck,
  paramsStore,
  setPluginParam,
  watchPluginParams,
  videoDoc,
  videoError,
  prefs,
  catalogue,
  prewrite,
  onConditions,
  timelineRef,
}: {
  surface: ProjectSurface;
  project: string;
  rev: string | null;
  store: EngineStore;
  engineRef: React.RefObject<EngineClient | undefined>;
  seek: (seconds: number) => void;
  commands: SurfaceCommands;
  setStatus: (s: string) => void;
  deck: Deck;
  paramsStore: PluginParamsStore;
  setPluginParam: (id: string, name: string, value: number) => void;
  watchPluginParams: (id: string, on: boolean) => void;
  videoDoc: WebVideoProject | null;
  videoError: string | null;
  prefs: CommandPrefs;
  catalogue: CommandInfo[];
  prewrite: (command: string) => void;
  onConditions: React.Dispatch<React.SetStateAction<ConditionCtx>>;
  timelineRef: React.RefObject<TimelineHandle | null>;
}) {
  // The DEVICE registry answers which component draws this type out here; a type with no
  // device drawer draws in k7's window, and the card says so (apps/k7-daw-frontend/packages/k7-sdk/docs/sdk.md).
  const drawer = deviceSurfaceDrawer(surface.surface.type);
  if (!drawer)
    return (
      <div className="flex flex-1 items-center justify-center p-8 text-center text-sm text-fg-2">
        {m["srf.drawnByShell"]({ type: surface.surface.type })}
      </div>
    );
  return drawer({
    surface,
    project,
    rev,
    store,
    engineRef,
    seek,
    commands,
    setStatus,
    deck,
    paramsStore,
    setPluginParam,
    watchPluginParams,
    videoDoc,
    videoError,
    prefs,
    catalogue,
    prewrite,
    onConditions,
    timelineRef,
  });
}

/** Back to the Surfaces page, as a plain link: this app's basename ends at the PROJECT, so there is no route from here to there. */
function Back() {
  return (
    <a
      href={surfacesUrl(window.location.origin, "k7")}
      title={m["extsrf.allSurfaces"]()}
      className="flex shrink-0 items-center gap-1 rounded px-1 py-0.5 text-xs text-fg-2 hover:bg-row"
    >
      <ChevronLeft className="h-4 w-4" />
      Surfaces
    </a>
  );
}

/** The one layout this screen has besides the surface: something centred, on its own. */
function Centre({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 bg-ground p-6 text-fg-2">
      {children}
    </div>
  );
}
