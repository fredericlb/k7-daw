// HomeFrame.tsx — what every home page shares: the command bar, the question host, the toasts, and the floating tab bar.
// `HomeFrame` is held above the routes, so a camera mode or an armed delete survives a page change; `HomeShell` is each
// page's layout. The tabs are LINKS, one address per page. See ../../docs/projects.md, "The home frame".

import { Toaster } from "@studio/project-shell/toast";
import {
  Folders,
  LibraryBig,
  ListChecks,
  type LucideIcon,
  Settings,
  TabletSmartphone,
} from "@studio/ui/icons";
import { TabBar, TabBarItem } from "@studio/ui/tab-bar";
import { lazy, type ReactNode, Suspense, useMemo, useState } from "react";
import { useHref, useLocation, useNavigate } from "react-router-dom";
import { AskHostK7 } from "../AskHostK7";
import { m } from "../paraglide/messages.js";
import { LIBRARIES_ROUTE, SETTINGS_ROUTE, SURFACES_ROUTE, TASKS_ROUTE } from "./api";
import { HomeCommandBar, HomeCommandsButton } from "./HomeCommandBar";
import { HelpContext } from "./homeHelp";
import { usePluginScanToast } from "./usePluginScanToast";

/** The documentation page is the project shell's; the home pages fetch it only when someone asks for it. */
const HelpPage = lazy(() =>
  import("../components/HelpPage").then((m) => ({ default: m.HelpPage })),
);

export function HomeFrame({ children }: { children: ReactNode }) {
  usePluginScanToast();
  const [help, setHelp] = useState(false);
  const openHelp = useMemo(() => () => setHelp(true), []);
  return (
    <HelpContext.Provider value={openHelp}>
      <HomeCommandBar>
        {/* Without this, `askConfirm` answers NO with no host to draw on and every destructive button silently does nothing. */}
        <AskHostK7 />
        <Toaster
          dismissLabel={m["common.dismiss"]()}
          regionLabel={m["toast.region"]()}
        />
        {children}
        {help && (
          <div className="fixed inset-0 z-50 bg-background">
            <Suspense fallback={null}>
              <HelpPage onBack={() => setHelp(false)} />
            </Suspense>
          </div>
        )}
      </HomeCommandBar>
    </HelpContext.Provider>
  );
}

/** A tab is an address: a plain click is a router move, a modified one opens like any link. */
function HomeTab({
  icon: Icon,
  label,
  route,
}: {
  icon: LucideIcon;
  label: ReactNode;
  route: string;
}) {
  const href = useHref(route);
  const navigate = useNavigate();
  const path = useLocation().pathname;
  // Settings stays selected on its sub-pages, such as the Engine page.
  const here =
    path === route || (route !== "/" && path.startsWith(`${route}/`));
  return (
    <TabBarItem
      href={href}
      icon={<Icon />}
      label={label}
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
          icon={TabletSmartphone}
          label={m["home.surfaces"]()}
          route={SURFACES_ROUTE}
        />
        <HomeTab
          icon={ListChecks}
          label={m["home.tasks"]()}
          route={TASKS_ROUTE}
        />
        <HomeTab
          icon={LibraryBig}
          label={m["home.library"]()}
          route={LIBRARIES_ROUTE}
        />
        <HomeTab
          icon={Settings}
          label={m["home.settings"]()}
          route={SETTINGS_ROUTE}
        />
      </TabBar>
    </div>
  );
}

/** The page frame: the page's own column, the tab bar floating over its foot. */
export function HomeShell({
  children,
  tabs = true,
}: {
  children: ReactNode;
  /** A sub-page (App settings) leaves the tab bar out and carries its own Back. */
  tabs?: boolean;
}) {
  return (
    <div className="flex h-full flex-col bg-background text-foreground">
      {/* `contain: layout` holds a page's fixed docks and pop-ins inside its column. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col [contain:layout]">
        {children}
      </div>
      {tabs && <HomeTabs />}
    </div>
  );
}
