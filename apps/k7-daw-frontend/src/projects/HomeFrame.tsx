// HomeFrame.tsx — what every home page shares: the command bar, the question host, the toasts, and the tabs (a sidebar, or a phone's floating bar).
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
import { cn } from "@studio/ui/lib/utils";
import { TabBar, TabBarItem, TabSidebar } from "@studio/ui/tab-bar";
import { lazy, type ReactNode, Suspense, useMemo, useState } from "react";
import { useHref, useLocation, useNavigate } from "react-router-dom";
import k7Logo from "../../icon/k7.svg";
import { AskHostK7 } from "../AskHostK7";
import { m } from "../paraglide/messages.js";
import { useIsMobile } from "../useIsMobile";
import {
  LIBRARIES_ROUTE,
  SETTINGS_ROUTE,
  SURFACES_ROUTE,
  TASKS_ROUTE,
} from "./api";
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
  row,
}: {
  icon: LucideIcon;
  label: ReactNode;
  route: string;
  row: boolean;
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
      row={row}
      className={row ? undefined : "max-sm:w-[68px]"}
      onClick={(e) => {
        if (e.defaultPrevented || e.button !== 0) return;
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
        e.preventDefault();
        navigate(route);
      }}
    />
  );
}

/** The five tabs: a phone's floating bar with the command-bar button beside it, else a sidebar with the logo and that button at its foot. */
function HomeTabs() {
  const phone = useIsMobile();
  const tabs = (
    <>
      <HomeTab
        icon={Folders}
        label={m["home.projects"]()}
        route="/"
        row={!phone}
      />
      <HomeTab
        icon={TabletSmartphone}
        label={m["home.surfaces"]()}
        route={SURFACES_ROUTE}
        row={!phone}
      />
      <HomeTab
        icon={ListChecks}
        label={m["home.tasks"]()}
        route={TASKS_ROUTE}
        row={!phone}
      />
      <HomeTab
        icon={LibraryBig}
        label={m["home.library"]()}
        route={LIBRARIES_ROUTE}
        row={!phone}
      />
      <HomeTab
        icon={Settings}
        label={m["home.settings"]()}
        route={SETTINGS_ROUTE}
        row={!phone}
      />
    </>
  );
  if (phone)
    return (
      <div className="pointer-events-none fixed inset-x-0 bottom-7 z-(--k7-z-bar) flex justify-center px-4">
        <TabBar
          label={m["home.nav"]()}
          className="pointer-events-auto"
          trailing={<HomeCommandsButton />}
        >
          {tabs}
        </TabBar>
      </div>
    );
  return (
    <TabSidebar
      label={m["home.nav"]()}
      className="fixed top-4 bottom-4 left-4 z-(--k7-z-bar)"
      foot={
        <>
          <img src={k7Logo} alt="k7" className="h-9 w-auto" draggable={false} />
          <HomeCommandsButton small />
        </>
      }
    >
      {tabs}
    </TabSidebar>
  );
}

/** The page frame: the page's own column, beside the sidebar or with a phone's tab bar floating over its foot. */
export function HomeShell({ children }: { children: ReactNode }) {
  const phone = useIsMobile();
  return (
    <div
      className={cn(
        "flex h-full flex-col bg-background text-foreground",
        !phone && "pl-[calc(var(--k7-sidebar-w)+32px)]",
      )}
    >
      {/* `contain: layout` holds a page's fixed docks and pop-ins inside its column. */}
      <div className="flex min-h-0 min-w-0 flex-1 flex-col [contain:layout]">
        {children}
      </div>
      <HomeTabs />
    </div>
  );
}
