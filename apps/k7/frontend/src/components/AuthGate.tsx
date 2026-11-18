// AuthGate.tsx — the OUTERMOST gate: the machine's local user must hold a session before anything else mounts,
// since the engine chooser and the file API are session-protected and would 401 for an anonymous tab.
// There is no sign-in screen: an anonymous tab is sent to the backend's `/auth/local` and comes back signed in.

import { GateWait } from "@studio/project-shell/progress";
import { Button } from "@studio/ui/button";
import { SignInScreen } from "@studio/ui/sign-in-screen";
import { type ReactNode, useEffect, useState } from "react";
import { type AuthUser, api } from "../api";
import { localApp } from "../localApp";
import { m } from "../paraglide/messages.js";
import { LocaleSwitcher } from "./LocaleSwitcher";

// The machine's local user, minted by its own backend only.
const LOCAL_SIGN_IN = "/auth/local";

// getMe answers null for a 401 and THROWS for anything else; off a device both are `down`.
type Phase =
  | { k: "checking" }
  | { k: "down"; detail: string }
  | { k: "authed"; user: AuthUser };

export function AuthGate({ children }: { children: ReactNode }) {
  const [phase, setPhase] = useState<Phase>({ k: "checking" });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let cancelled = false;
    api
      .getMe()
      .then(async (user) => {
        if (cancelled) return;
        if (user) return setPhase({ k: "authed", user });
        // Nobody signed in is the machine's local user, which only its own backend can start.
        if (await localApp()) {
          const back = encodeURIComponent(location.pathname + location.search);
          return location.replace(`${LOCAL_SIGN_IN}?return_to=${back}`);
        }
        setPhase({ k: "down", detail: "/api/local: no answer" });
      })
      .catch(
        (e: Error) =>
          !cancelled && setPhase({ k: "down", detail: e.message ?? String(e) }),
      );
    return () => {
      cancelled = true;
    };
  }, [attempt]);

  if (phase.k === "authed") return <>{children}</>;

  if (phase.k === "checking")
    return <GateWait label={m["startup.signingIn"]()} />;

  // The service that mints the session is down.
  return (
    <SignInScreen
      title="k7"
      description={m["auth.unreachable"]()}
      footer={<LocaleSwitcher />}
      className="text-center"
    >
      <code className="mb-4 block max-w-full truncate rounded bg-muted px-2 py-1 text-xs text-muted-foreground">
        {phase.detail}
      </code>
      <Button
        className="w-full"
        onClick={() => {
          setPhase({ k: "checking" });
          setAttempt((n) => n + 1);
        }}
      >
        {m["auth.retry"]()}
      </Button>
    </SignInScreen>
  );
}
