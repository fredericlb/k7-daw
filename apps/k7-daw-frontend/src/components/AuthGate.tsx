// AuthGate.tsx — the OUTERMOST gate: the page must hold the backend's token before anything else mounts, since the
// engine chooser and the file API answer it alone. There is no sign-in screen: `/auth/me` hands the token over.

import { GateWait } from "@studio/project-shell/progress";
import { Button } from "@studio/ui/button";
import { SignInScreen } from "@studio/ui/sign-in-screen";
import { type ReactNode, useEffect, useState } from "react";
import { type AuthUser, api } from "../api";
import { m } from "../paraglide/messages.js";
import { LocaleSwitcher } from "./LocaleSwitcher";

// getMe answers null for a 401 and THROWS for anything else; both are `down`.
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
      .then((user) => {
        if (cancelled) return;
        setPhase(
          user
            ? { k: "authed", user }
            : { k: "down", detail: "/auth/me: no session" },
        );
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

  // The backend that hands the token over is down.
  return (
    <SignInScreen
      title="K7 DAW"
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
