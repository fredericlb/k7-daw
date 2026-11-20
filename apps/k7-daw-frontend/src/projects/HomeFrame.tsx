// HomeFrame.tsx — what every home page shares: the command bar, the question host, the toasts, and the floating tab bar.
// `HomeFrame` is held above the routes, so a camera mode or an armed delete survives a page change; `HomeShell` is each
// page's layout. The tabs are LINKS, one address per page. See ../../docs/projects.md, "The home frame".

import { Toaster } from "@studio/project-shell/toast";
import {
  Folders,
  LibraryBig,
  ListChecks,
  type LucideIcon,
  Snowflake,
} from "@studio/ui/icons";
import { TabBar, TabBarItem } from "@studio/ui/tab-bar";
import type { ReactNode } from "react";
import { useHref, useLocation, useNavigate } from "react-router-dom";
import { AskHostK7 } from "../AskHostK7";
import { m } from "../paraglide/messages.js";
import { FROZEN_CHAINS_ROUTE, LIBRARIES_ROUTE, TASKS_ROUTE } from "./api";
import { HomeCommandBar, HomeCommandsButton } from "./HomeCommandBar";
import { usePluginScanToast } from "./usePluginScanToast";

export function HomeFrame({ children }: { children: ReactNode }) {
  usePluginScanToast();
  return (
    <HomeCommandBar>
      {/* Without this, `askConfirm` answers NO with no host to draw on and every destructive button silently does nothing. */}
      <AskHostK7 />
      <Toaster
        dismissLabel={m["common.dismiss"]()}
        regionLabel={m["toast.region"]()}
      />
      {children}
    </HomeCommandBar>
  );
}

/** A tab is an address: a plain click is a router move, a modified one opens like any link. */
function HomeTab({
  icon: Icon,
  label,
  name,
  route,
}: {
  icon: LucideIcon;
  label: string;
  /** The accessible name when the label is shortened for the narrow bar. */
  name?: string;
  route: string;
}) {
  const href = useHref(route);
  const navigate = useNavigate();
  const here = useLocation().pathname === route;
  return (
    <TabBarItem
      href={href}
      icon={<Icon />}
      label={label}
      aria-label={name}
      selected={here}
      className="max-sm:w-[68px]"
      onClick={(e) => {
        if (e.defaultPrevented || e.button !== 0) return;
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
        e.preventDefault();
        navigate(route);
      }}
    />
  );
}

/** The floating tab bar and the round command-bar button beside it, centred at the window's foot. */
export function HomeTabs() {
  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-7 z-(--k7-z-bar) flex justify-center px-4">
      <TabBar
        label={m["home.nav"]()}
        className="pointer-events-auto"
        trailing={<HomeCommandsButton />}
      >
        <HomeTab icon={Folders} label={m["home.projects"]()} route="/" />
        <HomeTab
          icon={ListChecks}
          label={m["home.tasks"]()}
          route={TASKS_ROUTE}
        />
        <HomeTab
          icon={LibraryBig}
          label={m["home.libraries"]()}
          route={LIBRARIES_ROUTE}
        />
        <HomeTab
          icon={Snowflake}
          label={m["home.frozenShort"]()}
          name={m["frozen.title"]()}
          route={FROZEN_CHAINS_ROUTE}
        />
      </TabBar>
    </div>
  );
}

/** The page frame: the page's own column, the tab bar floating over its foot. */
export function HomeShell({ children }: { children: ReactNode }) {
  return (
    <div className="flex h-full flex-col bg-background text-foreground">
      {/* `contain: layout` holds a page's fixed docks and pop-ins inside its column. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col [contain:layout]">
        {children}
      </div>
      <HomeTabs />
    </div>
  );
}
