// SettingsPage.tsx — `/settings`, a page of its own: what a BROWSER may change for its account, under two tabs —
// General (the language, the chains, the AI key, the name and email that sign commits) and Devices (the devices paired with the engine).
// The engines and the server's own settings are pages of their own beside it, in the user
// menu. A page and not a panel: it is visited on purpose and rarely. See ../../docs/settings.md.

import { AskHostK7 } from "../AskHostK7";
import { IdentitySection } from "../git/IdentitySection";
import { m } from "../paraglide/messages.js";
import { HomeShell } from "../projects/HomeFrame";
import { PageFrame } from "../projects/PageFrame";
import { AiSection } from "./AiSection";
import { LanguageSection } from "./LanguageSection";
import { PairedDevicesSection } from "./PairedDevicesSection";
import { type SettingsTab, SettingsTabs } from "./SettingsTabs";

export function SettingsPage() {
  const general: SettingsTab = {
    key: "general",
    label: m["settings.tabGeneral"](),
    content: (
      <>
        <LanguageSection />
        <AiSection />
        <IdentitySection />
      </>
    ),
  };
  const devices: SettingsTab = {
    key: "devices",
    label: m["settings.tabDevices"](),
    content: (
      <>
        <PairedDevicesSection />
      </>
    ),
  };
  return (
    <HomeShell>
      {/* Every tab's confirmation is drawn here. */}
      <AskHostK7 />
      <PageFrame title={m["settings.title"]()} narrow>
        <SettingsTabs tabs={[general, devices]} />
      </PageFrame>
    </HomeShell>
  );
}
