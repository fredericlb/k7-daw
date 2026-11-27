// PairRequestsPrompt.tsx — on the engine's own machine, one card per LAN device asking to pair:
// its name, its address and the code it shows, with Approve and Deny. Non-blocking, top-right.
// A card leaves when the engine's list drops it, whoever answered (the app's panel, settings, expiry).
// Not the Toaster: a card holds the code and two answers, and it lives as long as the engine's list does.

import type { EngineClient, PairRequest } from "@studio/engine-browser";
import { Button } from "@studio/ui/button";
import { Smartphone } from "@studio/ui/icons";
import { m } from "../paraglide/messages.js";
import { usePairing } from "./usePairing";

export function PairRequestsPrompt({
  engine,
  connected,
}: {
  engine: EngineClient | null | undefined;
  connected: boolean;
}) {
  const { requests } = usePairing(engine, connected);
  if (!engine || !requests.length) return null;
  return (
    <div className="fixed top-(--toast-top,calc(var(--k7-header-h)+8px)) right-4 z-(--k7-z-menu) flex w-[min(var(--k7-toast-max-w),calc(100vw-2rem))] flex-col gap-2">
      {requests.map((r) => (
        <PairRequestCard key={r.requestId} engine={engine} request={r} />
      ))}
    </div>
  );
}

/** One request, as a card: also what the settings page lists, so both read the same. */
export function PairRequestCard({
  engine,
  request,
}: {
  engine: EngineClient;
  request: PairRequest;
}) {
  return (
    <div className="float rounded-(--k7-r-toast) p-3.5 text-fg-1">
      <div className="type-label flex items-center gap-2">
        <Smartphone className="h-4 w-4 shrink-0 text-fg-2" />
        <span className="truncate">
          {m["pairing.requestTitle"]({ name: request.name })}
        </span>
      </div>
      <p className="type-secondary mt-1 text-fg-2">
        {m["pairing.requestBody"]({ ip: request.ip })}
      </p>
      <div className="type-mono-timecode mt-2 text-2xl tracking-[0.3em]">
        {request.code}
      </div>
      <div className="mt-2 flex justify-end gap-2">
        <Button
          size="sm"
          variant="secondary"
          onClick={() => engine.denyPairing(request.requestId)}
        >
          {m["pairing.deny"]()}
        </Button>
        <Button
          size="sm"
          onClick={() => engine.approvePairing(request.requestId)}
        >
          {m["pairing.approve"]()}
        </Button>
      </div>
    </div>
  );
}
