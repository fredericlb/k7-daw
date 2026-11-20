// App.tsx — the page under k7. Once the phone's own backend answers, k7 covers it, full screen
// (AppViewsPlugin.kt), with this phone's engine settings inside its own. The page shows itself only
// while that backend is still starting or failed to (Backend.tsx), or while k7 does not load: then
// no page is there to reach settings in.

import { Button } from "@studio/ui/button";
import { useEffect, useState } from "react";
import { BackendPanel } from "./Backend";
import {
  AppViews,
  type BackendStatus,
  K7Runtime,
  type SupervisorState,
} from "./native";
import { Tray } from "./Tray";

export function App() {
  const [state, setState] = useState<SupervisorState | null>(null);
  const [engineExit, setEngineExit] = useState<number | null>(null);
  const [backend, setBackend] = useState<BackendStatus>({ state: "starting" });
  const [failed, setFailed] = useState<{ message: string } | null>(null);

  useEffect(() => {
    const handles = [
      K7Runtime.addListener("state", setState),
      K7Runtime.addListener("engineExit", ({ code }) => setEngineExit(code)),
      AppViews.addListener("loadError", setFailed),
      AppViews.addListener("backend", setBackend),
    ];
    // After the listeners: a frame between the two calls is then drawn twice rather than missed.
    K7Runtime.getState().then((known) => {
      if (known.state) setState(known.state);
      if (known.engineExit != null) setEngineExit(known.engineExit);
    });
    AppViews.backend().then(setBackend);
    return () => {
      for (const h of handles) h.then((l) => l.remove());
    };
  }, []);

  // The phone's own backend is what k7 loads from: its coming up is another chance for a load that failed.
  // biome-ignore lint/correctness/useExhaustiveDependencies: the backend changing IS the signal.
  useEffect(() => setFailed(null), [backend.state]);

  useEffect(() => {
    if (failed || backend.state !== "ready") {
      AppViews.hideAll();
      return;
    }
    AppViews.show().catch(() => {
      // A start that failed is said by `loadError` and `backend`.
    });
  }, [failed, backend.state]);

  const retry = () => {
    if (failed) AppViews.reload();
    setFailed(null);
  };

  return (
    <div className="min-h-dvh bg-background pt-[var(--safe-top)] text-foreground">
      <BackendPanel status={backend} />
      {failed && backend.state === "ready" ? (
        <section className="mx-auto flex w-full max-w-xl flex-col gap-2 px-4 pt-6">
          <p className="text-destructive text-sm">
            k7 did not load: {failed.message}
          </p>
          <Button className="self-start" onClick={retry}>
            Retry
          </Button>
        </section>
      ) : null}
      <Tray
        state={state}
        engineExit={engineExit}
        certFailed={!!failed?.message.includes("certificate")}
      />
    </div>
  );
}
