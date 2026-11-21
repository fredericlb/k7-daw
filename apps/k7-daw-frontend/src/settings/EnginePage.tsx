// EnginePage.tsx — `/settings/engine`, reached from App settings › Devices. On the desktop app, this machine's engine under two tabs:
// Engine (whether it runs, whether its network may use it unpaired, its slots) and Plugins; the audio interface is in Devices.
// What the command-line engine reads from its config file; the desktop app hands its user no such file. Anywhere else,
// the phone's own engine. See ../../docs/settings.md.

import { askConfirm } from "@studio/project-shell/ask";
import { Switch } from "@studio/ui/switch";
import { m } from "../paraglide/messages.js";
import { HomeShell } from "../projects/HomeFrame";
import { PageFrame } from "../projects/PageFrame";
import { AndroidEngineSection } from "./AndroidEngineSection";
import { onAndroid } from "./androidEngine";
import {
  type DesktopEngineSnapshot,
  engineCommand,
  useDesktopEngine,
  useOnDesktopEngine,
} from "./desktopEngine";
import { Action, Row, Section } from "./Section";
import { SettingsTabs } from "./SettingsTabs";
import { SlotsSection } from "./SlotsSection";

function statusLine(
  s: DesktopEngineSnapshot | null,
  down: string | null,
): string {
  if (down) return down;
  if (!s) return m["engine.waiting"]();
  if (s.status === "running")
    return m["engine.running"]({ port: String(s.enginePort) });
  if (s.status === "error") return m["engine.error"]({ error: s.error ?? "" });
  return s.userStopped ? m["engine.stoppedByYou"]() : m["engine.stopped"]();
}

export function EnginePage() {
  const here = useOnDesktopEngine();
  const state = useDesktopEngine();
  const s = state?.snapshot ?? null;
  const running = s?.status === "running";
  // Absent from an older supervisor, whose engine always asks pairing.
  const lanOpen = s?.lanOpen ?? false;
  // Stopping silences every device that plays through this engine, so it asks first.
  const stopEngine = async () => {
    const ok = await askConfirm({
      title: m["engine.stopTitle"](),
      body: m["engine.stopBody"](),
      confirmLabel: m["engine.stop"](),
      danger: true,
    });
    if (ok) engineCommand("engine.stop");
  };
  const engineTab = (
    <>
      <Section title={m["engine.title"]()} hint={m["engine.hint"]()}>
        <Row
          label={statusLine(s, state?.down ?? null)}
          detail={s?.engineId ? m["engine.id"]({ id: s.engineId }) : undefined}
        >
          {running ? (
            <>
              <Action onClick={() => engineCommand("engine.restart")}>
                {m["engine.restart"]()}
              </Action>
              <Action danger onClick={() => void stopEngine()}>
                {m["engine.stop"]()}
              </Action>
            </>
          ) : (
            <Action disabled={!s} onClick={() => engineCommand("engine.start")}>
              {m["engine.start"]()}
            </Action>
          )}
        </Row>
      </Section>

      <Section title={m["engine.lanTitle"]()} hint={m["engine.lanHint"]()}>
        <Row
          label={m["engine.lanOpen"]()}
          detail={lanOpen ? m["engine.lanOpenOn"]() : m["engine.lanOpenOff"]()}
        >
          <Switch
            checked={lanOpen}
            disabled={!s}
            onCheckedChange={(on) => engineCommand("lan.open", on ? "1" : "0")}
            aria-label={m["engine.lanOpen"]()}
          />
        </Row>
      </Section>
      <SlotsSection s={s} />
    </>
  );

  const pluginsTab = (
    <Section
      title={m["engine.pluginsTitle"]()}
      hint={m["engine.pluginsHint"]()}
    >
      <Row
        label={
          s?.pluginCount === undefined
            ? m["engine.scanning"]()
            : m["engine.pluginCount"]({ n: String(s.pluginCount) })
        }
      >
        <Action
          disabled={!running}
          onClick={() => engineCommand("plugins.rescan")}
        >
          {m["engine.rescan"]()}
        </Action>
      </Row>
      {(s?.plugins ?? []).map((p) => (
        <Row
          key={`${p.format}:${p.name}`}
          label={p.name}
          detail={[p.manufacturer, p.format].filter(Boolean).join(" · ")}
        />
      ))}
    </Section>
  );

  return (
    <HomeShell>
      <PageFrame title={m["engine.title"]()} narrow back>
        {here === undefined ? null : !here ? (
          <div className="flex min-w-0 flex-col gap-4">
            {/* On the phone the app is the engine's machine: this is where its engine is set. */}
            {onAndroid ? (
              <AndroidEngineSection />
            ) : (
              <p className="text-muted-foreground text-xs">
                {m["settings.trayHint"]()}
              </p>
            )}
          </div>
        ) : (
          <SettingsTabs
            tabs={[
              {
                key: "engine",
                label: m["engine.tabEngine"](),
                content: engineTab,
              },
              {
                key: "plugins",
                label: m["engine.tabPlugins"](),
                content: pluginsTab,
              },
            ]}
          />
        )}
      </PageFrame>
    </HomeShell>
  );
}
