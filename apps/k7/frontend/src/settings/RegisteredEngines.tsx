// RegisteredEngines.tsx — the engines this account may use at this origin, as the engine page lists them: the phone's
// own engine first inside the Android app, the hint that a computer's engine is set on that computer elsewhere,
// then the registry's card. On the tray it is a tab beside this machine's engine. See ../../docs/settings.md.

import { m } from "../paraglide/messages.js";
import { AndroidEngineSection } from "./AndroidEngineSection";
import { onAndroid } from "./androidEngine";
import { EnginesSettings } from "./EnginesSettings";
import { RemoteSlotsSection } from "./RemoteSlotsSection";

export function RegisteredEngines({ onTray }: { onTray: boolean }) {
  return (
    <>
      {/* On the phone the app is the engine's machine: this is where its engine is set. */}
      {onAndroid ? (
        <AndroidEngineSection />
      ) : (
        !onTray && (
          <p className="text-muted-foreground text-xs">
            {m["settings.trayHint"]()}
          </p>
        )
      )}
      <EnginesSettings />
      <RemoteSlotsSection />
    </>
  );
}
