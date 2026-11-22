// EngineGate.tsx — open the project at once and find this device's engine BESIDE it. The files are
// the hub's, reached with no engine bound; the engine is looked for meanwhile — registered,
// MEASURED, and taken when it is this device's own (`localEngineOf`). The editor's link waits for that, and a
// project with no engine stays open and silent, the bar saying so. The claim waits for the address
// to resolve, and a lost one leaves the project open under a banner. See ../../docs/shell.md.

import type { HostedFiles } from "@studio/engine-browser/access";
import { retryLoopback } from "@studio/engine-browser/probe";
import { type GateEngine, projectResolved } from "@studio/project-shell/gate";
import { parseLaunch, projectClaimKey } from "@studio/project-shell/launch";
import { GateWait } from "@studio/project-shell/progress";
import { Button } from "@studio/ui/button";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { engines } from "../api";
import {
  type LocalEngine,
  LocalEngineContext,
  localEngineOf,
} from "../localEngine";
import { m } from "../paraglide/messages.js";
import type { RankedEngine } from "../types";
import { EngineTakenBanner } from "./EngineTakenBanner";
import { holdsClaim, type Taken } from "./engineTaken";
import { ProjectGate } from "./ProjectGate";

/** How many more looks an owned engine that did not answer gets, and how far apart (docs/shell.md). */
const OWNED_RETRIES = 3;
const OWNED_RETRY_MS = 5_000;

export function EngineGate({
  /** The surface this tab was opened on, when the address named one — forwarded, not acted on: this gate's job is the engine. */
  external,
}: {
  external?: string | null;
} = {}) {
  // The hub's files, undefined while asked, null when this device hosts none (then the engine's own are needed).
  const [hosted, setHosted] = useState<HostedFiles | null | undefined>(
    undefined,
  );
  const [chosen, setChosen] = useState<RankedEngine | null>(null);
  // The first look for the engine is still running.
  const [searching, setSearching] = useState(true);
  // Somebody else took the claim: the project stays open, its link down, until it is taken back.
  const [taken, setTaken] = useState<Taken | null>(null);
  const takenRef = useRef<Taken | null>(null);
  // The address resolved to a project that exists — what a claim waits for (@studio/project-shell docs/launch.md).
  const [resolved, setResolved] = useState(false);
  // Bumped when the engine's files differ from the hub's: the editor is remounted onto them.
  const [generation, setGeneration] = useState(0);
  /** The session fell asleep, and this gate draws one consequence: DROP THE LEASE, since a tab with no socket would only squat the machine. */
  const [asleep, setAsleep] = useState(false);

  // The engine as the editor's link reads it, and whoever is waiting for the first answer.
  const chosenRef = useRef<RankedEngine | null>(null);
  const waiters = useRef<((r: RankedEngine | null) => void)[]>([]);
  const settled = useRef(false);

  // The files open before the engine is found, read by `take` to tell an engine whose files are not the hub's.
  const hostedRef = useRef<HostedFiles | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    void engines
      .hostedFiles()
      .catch(() => null)
      .then((h) => {
        hostedRef.current = h;
        if (live) setHosted(h);
      });
    return () => {
      live = false;
    };
  }, []);

  /** Which project this tab is claiming, read from the address exactly as the router's basename is; the null branch keeps a rule `main.tsx` enforces. */
  const claimProject = useMemo(() => {
    const target = parseLaunch(
      window.location.pathname,
      "k7",
      import.meta.env.BASE_URL,
    );
    return target ? projectClaimKey(target) : null;
  }, []);

  /** Take `r`; an engine whose files are not the hub's moves the project there, so the editor is remounted onto them. */
  const take = useCallback((r: RankedEngine): boolean => {
    if (!r.wsUrl || !r.fileBase || r.token === null) return false;
    const before = chosenRef.current?.fileBase ?? hostedRef.current?.fileBase;
    const moves = !!before && before !== r.fileBase;
    chosenRef.current = r;
    for (const w of waiters.current.splice(0)) w(r);
    setChosen(r);
    if (moves) {
      takenRef.current = null;
      setTaken(null);
      setResolved(false);
      setGeneration((g) => g + 1);
    }
    return true;
  }, []);

  // The claim is a LEASE this tab keeps alive ON THIS PROJECT (@studio/engine-browser/docs/lease.md); asleep, the cleanup releases it.
  const heldId = chosen?.engine.id ?? null;
  const isTaken = taken !== null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: `chosen` is read only through `heldId`, which is what says it changed.
  useEffect(() => {
    if (
      !chosen ||
      !holdsClaim({ engine: true, resolved, asleep, taken: isTaken })
    )
      return;
    return engines.hold(chosen, {
      project: claimProject,
      onLost: (heldBy, why) => {
        const t = { heldBy, why: why.reason, tookFor: why.tookFor };
        takenRef.current = t;
        setTaken(t);
      },
    });
  }, [heldId, claimProject, asleep, resolved, isTaken]);

  /** Taking it back IS opening the project again: the gate resolves anew, and the claim that follows displaces the other tab. */
  const takeBack = useCallback(() => {
    takenRef.current = null;
    setTaken(null);
    setResolved(false);
    setGeneration((g) => g + 1);
  }, []);

  // Looks left for an engine this device owns that did not answer: a page still loading can hold its reply past the probe's 3 s.
  const retriesLeft = useRef(OWNED_RETRIES);
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(
    () => () => {
      if (retryTimer.current) clearTimeout(retryTimer.current);
    },
    [],
  );

  /** Looks for the engine on this machine; a step that threw answers like none being usable: the project stays open and silent. */
  const look = useCallback(async (): Promise<void> => {
    try {
      const { ranked } = await engines.find();
      const here = localEngineOf(ranked);
      if (here) take(here);
      else {
        // The registry names an engine of this account that was silent: looked for again, a few times, rather than left silent.
        const silent = ranked.filter(
          (r) => r.engine.access === "owner" && r.token === null,
        );
        if (retriesLeft.current > 0 && silent.length) {
          retriesLeft.current--;
          retryTimer.current = setTimeout(() => {
            if (chosenRef.current) return;
            // The loopback that just missed rests two minutes otherwise, and is the very address this look is for.
            for (const r of silent) retryLoopback(r.engine);
            void lookRef.current();
          }, OWNED_RETRY_MS);
        }
        // Said once per look, to the console a phone's logcat carries: what was offered, and why none is this device's.
        console.info(
          "[engine-gate] no engine of this device among",
          JSON.stringify(
            ranked.map((r) => ({
              id: r.engine.id,
              wsUrl: r.wsUrl,
              answered: r.token !== null,
              networkMs: r.latency?.networkMs ?? null,
              access: r.engine.access,
              addresses: r.engine.addresses,
            })),
          ),
        );
      }
    } catch {
      // No engine: the bar says so, and the engine page starts one.
    } finally {
      settled.current = true;
      // A link still waiting hears "none" now rather than at its deadline.
      for (const w of waiters.current.splice(0)) w(chosenRef.current);
      setSearching(false);
    }
  }, [take]);
  const lookRef = useRef(look);
  lookRef.current = look;

  useEffect(() => {
    // StrictMode runs this twice in dev; both probes are harmless, and a ref guard would strand the search.
    void look();
  }, [look]);

  const searchAgain = useCallback(() => {
    if (chosenRef.current) return;
    setSearching(true);
    void look();
  }, [look]);

  // Asked on every dial: the choice once there is one, else a wait for it — which the link's own deadline bounds.
  const linkUrls = useRef(
    new Map<string, () => string | Promise<string | null>>(),
  );
  const engineUrl = useCallback(async (): Promise<string | null> => {
    // Nobody to dial while someone else holds the claim: the link stays down rather than drive their project.
    if (takenRef.current) return null;
    const r =
      chosenRef.current ??
      (settled.current
        ? null
        : await new Promise<RankedEngine | null>((resolve) =>
            waiters.current.push(resolve),
          ));
    if (!r) return null;
    let url = linkUrls.current.get(r.engine.id);
    if (!url) {
      url = engines.linkUrl(r);
      linkUrls.current.set(r.engine.id, url);
    }
    return url();
  }, []);

  // The files are the chosen engine's way in when it has one, else the hub's own with no engine bound.
  const gateEngine = useMemo<GateEngine | null>(() => {
    // A session open past its twelve-hour ticket asks for a new one rather than failing every read — apps/k7-daw-frontend/packages/project-shell/docs/fileApi.md.
    const renewTicket = () => engines.freshTicket();
    if (chosen?.fileBase)
      return chosen.filesHosted ? { ...chosen, renewTicket } : chosen;
    if (!hosted) return null;
    return {
      fileBase: hosted.fileBase,
      token: null,
      fileToken: hosted.fileToken,
      engine: { id: "" },
      renewTicket,
    };
  }, [chosen, hosted]);

  const local = useMemo<LocalEngine>(
    () => ({ detached: isTaken, searchAgain }),
    [isTaken, searchAgain],
  );

  // Nothing to open the files with yet: the hub still being asked, or no hub and no engine found.
  if (!gateEngine) {
    if (hosted === undefined || searching)
      return (
        <GateWait
          label={
            hosted === undefined
              ? m["startup.files"]()
              : m["startup.searching"]()
          }
        />
      );
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 bg-background text-foreground">
        <p className="text-muted-foreground text-sm">{m["engines.none"]()}</p>
        <Button onClick={searchAgain}>{m["common.refresh"]()}</Button>
      </div>
    );
  }

  return (
    <LocalEngineContext.Provider value={local}>
      <ProjectGate
        key={generation}
        // No engine while it is someone else's: the editor reads as a project with none, its files still open.
        engine={taken ? null : chosen}
        gateEngine={gateEngine}
        engineUrl={engineUrl}
        engineSearching={searching && !chosen}
        external={external}
        onIdle={() => setAsleep(true)}
        // Resuming reopens the project, so it takes the claim back like the banner's button.
        onResume={() => {
          takenRef.current = null;
          setTaken(null);
          setAsleep(false);
        }}
        onPhase={(k) => setResolved(projectResolved(k))}
      />
      {taken && <EngineTakenBanner taken={taken} onTakeBack={takeBack} />}
    </LocalEngineContext.Provider>
  );
}
