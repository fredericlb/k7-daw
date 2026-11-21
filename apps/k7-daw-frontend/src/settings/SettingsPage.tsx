// SettingsPage.tsx — `/settings`, App settings: what a BROWSER may change for its account, under three tabs — General
// (the language, the commit identity, the AI key, the separation chains), Devices (the audio interface, the devices
// paired with the engine, the row that opens its Engine page), Downloads (what is coming down, the engine's instruments) and About. A page and not a panel: it is visited on purpose and
// rarely. See ../../docs/settings.md.

import { AudioLines, ChevronRight } from "@studio/ui/icons";
import { Link } from "react-router-dom";
import { AboutSection } from "../about/AboutSection";
import { GitHubSection } from "../git/GitHubSection";
import { IdentitySection } from "../git/IdentitySection";
import { m } from "../paraglide/messages.js";
import { ENGINE_SETTINGS_ROUTE } from "../projects/api";
import { HomeShell } from "../projects/HomeFrame";
import { PageFrame } from "../projects/PageFrame";
import { AiSection } from "./AiSection";
import { AudioInterfaceSection } from "./AudioInterfaceSection";
import { ChainsSection } from "./ChainsSection";
import { DownloadsSection } from "./DownloadsSection";
import {
  engineCommand,
  useDesktopEngine,
  useOnDesktopEngine,
} from "./desktopEngine";
import { InstrumentsSection } from "./InstrumentsSection";
import { LanguageSection } from "./LanguageSection";
import { PairedDevicesSection } from "./PairedDevicesSection";
import { Section } from "./Section";
import { type SettingsTab, SettingsTabs } from "./SettingsTabs";

export function SettingsPage() {
  const snapshot = useDesktopEngine()?.snapshot ?? null;
  const onDesktop = useOnDesktopEngine();
  const engineId = snapshot?.engineId ?? "";
  const general: SettingsTab = {
    key: "general",
    label: m["settings.tabGeneral"](),
    content: (
      <>
        <LanguageSection />
        <IdentitySection />
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
        {onDesktop && <AudioInterfaceSection s={snapshot} />}
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
  const services: SettingsTab = {
    key: "services",
    label: m["settings.tabServices"](),
    content: <GitHubSection />,
  };
  const downloads: SettingsTab = {
    key: "downloads",
    label: m["settings.tabDownloads"](),
    content: (
      <>
        <DownloadsSection />
        {onDesktop && (
          <InstrumentsSection snapshot={snapshot} send={engineCommand} />
        )}
      </>
    ),
  };
  const about: SettingsTab = {
    key: "about",
    label: m["settings.tabAbout"](),
    content: <AboutSection />,
  };
  return (
    <HomeShell tabs={false}>
      <PageFrame
        title={m["settings.title"]()}
        summary={m["settings.scope"]()}
        narrow
        back
      >
        <SettingsTabs
          label={m["settings.title"]()}
          tabs={[general, devices, services, downloads, about]}
        />
      </PageFrame>
    </HomeShell>
  );
}
