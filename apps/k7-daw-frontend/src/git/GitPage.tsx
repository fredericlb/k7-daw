// GitPage.tsx — the frame of the home's git pages (Sync settings, Clone a repository): a full screen over the home, a Back
// to Projects, a title, and the services as tabs. Escape is Back, as on every screen reached from the home (docs/git.md).

import { Button } from "@studio/ui/button";
import { ChevronLeft } from "@studio/ui/icons";
import { useModalFocus } from "@studio/ui/modal-focus";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@studio/ui/tabs";
import { type ReactNode, useRef } from "react";
import { createPortal } from "react-dom";
import { m } from "../paraglide/messages.js";
import { PAGE_COLUMN } from "../projects/PageFrame";
import { useIsMobile } from "../useIsMobile";

export interface ServiceTab {
  key: string;
  label: string;
  /** Nothing set up for this service yet: the tab says so to a screen reader. */
  notSetUp?: boolean;
  content: ReactNode;
}

export function GitPage({
  title,
  subtitle,
  region,
  intro,
  tabs,
  tab,
  onTab,
  busy = false,
  onBack,
}: {
  title: string;
  subtitle?: string;
  /** The region's accessible name: what the page is about. */
  region: string;
  /** One sentence over every tab: what the page does. */
  intro: string;
  tabs: ServiceTab[];
  tab: string;
  onTab: (key: string) => void;
  /** A call is running: Back and Escape wait for it. */
  busy?: boolean;
  onBack: () => void;
}) {
  const ref = useRef<HTMLElement>(null);
  useModalFocus(ref);
  const phone = useIsMobile();
  // On the body: the home's column holds its fixed children under the tab bar (`contain: layout`).
  return createPortal(
    <section
      ref={ref}
      aria-label={region}
      // Beside the home's sidebar, which stays: only a phone's page covers the whole window.
      className={`fixed inset-y-0 right-0 z-50 overflow-y-auto bg-background text-foreground ${phone ? "left-0" : "left-[calc(var(--k7-sidebar-w)+32px)]"}`}
      onKeyDown={(e) => {
        // An open menu or question takes its own Escape first.
        if (e.key !== "Escape" || e.defaultPrevented || busy) return;
        e.preventDefault();
        onBack();
      }}
    >
      <div className={`${PAGE_COLUMN} max-w-2xl`}>
        <header className="flex flex-col gap-3">
          <Button
            variant="ghost"
            className="self-start"
            aria-label={m["git.page.back"]()}
            disabled={busy}
            onClick={onBack}
          >
            <ChevronLeft className="size-5" />
            {m["home.projects"]()}
          </Button>
          <div className="flex flex-col gap-1">
            <h1 className="type-large-title">{title}</h1>
            {subtitle && <p className="type-secondary text-fg-2">{subtitle}</p>}
          </div>
        </header>
        <Tabs value={tab} onValueChange={onTab}>
          <TabsList aria-label={m["git.page.services"]()}>
            {tabs.map((t) => (
              <TabsTrigger
                key={t.key}
                value={t.key}
                aria-label={
                  t.notSetUp
                    ? m["git.page.notSetUp"]({ service: t.label })
                    : t.label
                }
              >
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>
          <p className="type-secondary mt-4 text-fg-2">{intro}</p>
          {tabs.map((t) => (
            <TabsContent
              key={t.key}
              value={t.key}
              aria-label={t.label}
              className="mt-4 flex min-w-0 flex-col gap-4"
            >
              {t.content}
            </TabsContent>
          ))}
        </Tabs>
      </div>
    </section>,
    document.body,
  );
}
