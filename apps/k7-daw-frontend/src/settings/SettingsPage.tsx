// SettingsPage.tsx — `/settings`, App settings: what a BROWSER may change for its account, under two tabs — General
// (the language, the commit identity, About, the AI key, the separation chains) and Devices (the devices paired with the
// engine, and the row that opens this machine's Engine page). A page and not a panel: it is visited on purpose and
// rarely. See ../../docs/settings.md.

import { AudioLines, ChevronRight } from "@studio/ui/icons";
import { Link } from "react-router-dom";
import { AboutSection } from "../about/AboutSection";
import { IdentitySection } from "../git/IdentitySection";
import { m } from "../paraglide/messages.js";
import { ENGINE_SETTINGS_ROUTE } from "../projects/api";
import { HomeShell } from "../projects/HomeFrame";
import { PageFrame } from "../projects/PageFrame";
import { AiSection } from "./AiSection";
import { ChainsSection } from "./ChainsSection";
import { useDesktopEngine } from "./desktopEngine";
import { LanguageSection } from "./LanguageSection";
import { PairedDevicesSection } from "./PairedDevicesSection";
import { Section } from "./Section";
import { type SettingsTab, SettingsTabs } from "./SettingsTabs";

export function SettingsPage() {
  const engineId = useDesktopEngine()?.snapshot?.engineId ?? "";
  const general: SettingsTab = {
    key: "general",
    label: m["settings.tabGeneral"](),
    content: (
      <>
        <LanguageSection />
        <IdentitySection />
        <AboutSection />
        <AiSection />
        <ChainsSection engineId={engineId} />
      </>
    ),
  };
  const devices: SettingsTab = {
    key: "devices",
    label: m["settings.tabDevices"](),
    content: (
      <>
        <PairedDevicesSection />
        <Section title={m["home.engine"]()}>
          <Link
            to={ENGINE_SETTINGS_ROUTE}
            className="type-body flex min-h-11 items-center gap-3 px-3.5 py-2.5 outline-none hover:bg-on-fill focus-visible:shadow-(--k7-focus-ring)"
          >
            <AudioLines className="size-5 shrink-0 text-primary-text" />
            <span className="min-w-0 flex-1">
              <span className="block truncate">
                {m["settings.engineRow"]()}
              </span>
              <span className="type-status block truncate text-fg-3">
                {m["settings.engineRowDetail"]()}
              </span>
            </span>
            <ChevronRight className="size-4 shrink-0 text-fg-3" aria-hidden />
          </Link>
        </Section>
      </>
    ),
  };
  return (
    <HomeShell tabs={false}>
      <PageFrame
        title={m["settings.title"]()}
        summary={m["settings.scope"]()}
        narrow
        cog={false}
        back
      >
        <SettingsTabs label={m["settings.title"]()} tabs={[general, devices]} />
      </PageFrame>
    </HomeShell>
  );
}
