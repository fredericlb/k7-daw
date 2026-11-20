// Backend.tsx — the phone's own k7 while it is not up yet (BackendHost.kt): a first run unpacks for a few seconds,
// and a start that failed says why with a Retry, since k7 cannot open on a backend that never answered.
// The wait is the startup splash, full screen over the tray, on the colour the native launch screen drew (k7's docs/icons.md).

import { Button } from "@studio/ui/button";
import splash from "../../../k7-daw-frontend/icon/k7-splash.svg";
import { AppViews, type BackendStatus } from "./native";

export function BackendPanel({ status }: { status: BackendStatus }) {
  if (status.state === "ready") return null;
  if (status.state === "failed") {
    return (
      <section className="mx-auto flex w-full max-w-xl flex-col gap-3 px-4 pt-10">
        <h1 className="text-lg font-medium">k7 could not start</h1>
        <p className="text-sm text-destructive">{status.message}</p>
        <Button
          className="self-start"
          onClick={() => AppViews.restartBackend()}
        >
          Retry
        </Button>
      </section>
    );
  }
  const pct =
    status.step === "unpacking" && status.progress != null
      ? Math.round(status.progress * 100)
      : null;
  // One screen with the web splash (apps/k7-daw-frontend/index.html): the cassette, the ring, then two reserved lines.
  const line =
    status.step === "unpacking"
      ? `Unpacking k7… ${pct ?? 0}%`
      : status.step === "launching"
        ? "Starting k7…"
        : "Preparing k7…";
  const hint =
    status.step === "unpacking"
      ? "only on the first run after an install"
      : null;
  return (
    <section
      role="status"
      aria-label="Starting k7"
      className="fixed inset-0 z-50 flex flex-col items-center justify-center gap-6 bg-[#07031F] px-6"
    >
      <img src={splash} alt="" className="w-[280px]" />
      {/* The ring's size and colours are the web splash's: the two are one screen, handed over mid-spin. */}
      <span
        aria-hidden="true"
        className="size-7 animate-spin rounded-full border-2 border-[#2a2450] border-t-[#b7b0d9]"
      />
      {/* Two lines whatever the step says, so a longer line moves neither the cassette nor the ring. */}
      <div className="flex min-h-[2.8em] w-[min(80vw,420px)] flex-col items-center text-center text-sm leading-[1.4] text-[#b7b0d9] tabular-nums">
        <p>{line}</p>
        {hint ? <p className="text-xs opacity-70">{hint}</p> : null}
      </div>
    </section>
  );
}
