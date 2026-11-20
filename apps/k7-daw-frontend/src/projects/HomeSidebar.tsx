// HomeSidebar.tsx — the left sidebar of the home page and of the pages that hang off it, /libraries,
// /tasks, /settings and /about: Projects, Tasks, Libraries and Frozen chains as entries, the user menu
// at its foot. All are routes, so going between them reloads
// nothing. It is an ICON RAIL wherever the `wide` step is not reached — a phone, a tablet held upright — and folds away
// entirely on request. Inside a project it is a DRAWER (`HomeDrawer`): the router's basename is the project there, so its
// entries are full addresses (appNav.ts). See ../../docs/projects.md.

import { projectsUrl } from "@studio/project-shell/launch";
import { Button } from "@studio/ui/button";
import {
  AudioLines,
  Folders,
  Info,
  LibraryBig,
  ListChecks,
  type LucideIcon,
  Snowflake,
} from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { UserMenu } from "@studio/ui/user-menu";
import { type RefObject, useLayoutEffect, useState } from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";
import k7Logo from "../../icon/k7.svg";
import { go } from "../appNav";
import { DownloadsButton } from "../components/DownloadsButton";
import { m } from "../paraglide/messages.js";
import { usePopin } from "../popins";
import { VERSION } from "../version";
import {
  ABOUT_ROUTE,
  ENGINE_SETTINGS_ROUTE,
  FROZEN_CHAINS_ROUTE,
  LIBRARIES_ROUTE,
  SETTINGS_ROUTE,
  TASKS_ROUTE,
} from "./api";
import { useMenuAccount } from "./menuAccount";

/** A home route as a full address, which appNav draws in place, for a sidebar drawn outside the home router. */
const homeHref = (route: string): string =>
  new URL(route.slice(1), projectsUrl(window.location.href)).href;

/** The sidebar over a project: slides in from the left; the bar's own button, a click beside it or Escape closes it. */
export function HomeDrawer({
  open,
  onClose,
  above,
}: {
  open: boolean;
  onClose: () => void;
  /** The project bar: the drawer stops at its top edge, so the bar stays whole and in reach. */
  above: RefObject<HTMLElement | null>;
}) {
  usePopin(open, onClose);
  const bottom = useSpaceBelow(open ? above : null);
  if (!open) return null;
  return (
    <div className="fixed inset-x-0 top-0 z-50 flex" style={{ bottom }}>
      <HomeSidebar away />
      <button
        type="button"
        aria-label={m["common.close"]()}
        className="flex-1 cursor-default bg-black/40"
        onClick={onClose}
      />
    </div>
  );
}

/** How far the element's top edge is from the window's bottom — measured, since the bar's height depends on what it holds. */
function useSpaceBelow(ref: RefObject<HTMLElement | null> | null): number {
  const [space, setSpace] = useState(0);
  useLayoutEffect(() => {
    const el = ref?.current;
    if (!el) return;
    const measure = () =>
      setSpace(
        Math.max(0, window.innerHeight - el.getBoundingClientRect().top),
      );
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    window.addEventListener("resize", measure);
    return () => {
      ro.disconnect();
      window.removeEventListener("resize", measure);
    };
  }, [ref]);
  return space;
}

interface Entry {
  label: string;
  icon: LucideIcon;
  route: string;
  active: boolean;
}

/** A LINK, not a button: an entry is an address, so it opens in a new tab and copies like one. */
function SidebarEntry({
  entry: { label, icon: Icon, route, active },
  away,
}: {
  entry: Entry;
  away?: boolean;
}) {
  const inner = (
    <>
      <Icon className={cn("h-4 w-4 shrink-0", active && "text-primary-text")} />
      <span
        className={cn(
          "min-w-0 flex-1 truncate text-left",
          away ? "inline" : "hidden wide:inline",
        )}
      >
        {label}
      </span>
    </>
  );
  return (
    <Button
      variant="ghost"
      size="sm"
      title={label}
      aria-current={active ? "page" : undefined}
      asChild
      className={cn(
        away ? "justify-start" : "justify-center wide:justify-start",
        // The lit entry is the one place the sidebar wears the accent: violet means "you are here" (../../docs/projects.md).
        active &&
          "bg-primary/15 text-foreground hover:bg-primary/20 hover:text-foreground",
      )}
    >
      {away ? (
        <a href={homeHref(route)}>{inner}</a>
      ) : (
        <Link to={route}>{inner}</Link>
      )}
    </Button>
  );
}

export function HomeSidebar({
  away,
}: {
  /** Drawn over a project: full-address links, nothing lit, never folded to the rail. */
  away?: boolean;
}) {
  const routerNavigate = useNavigate();
  const navigate = (route: string) =>
    away ? void go(homeHref(route)) : routerNavigate(route);
  // Over a project the address is the project's, so no entry is "here".
  const pathname = useLocation().pathname;
  const here = (route: string) => !away && pathname === route;

  // The menu's heading: the login on the server, or "Local" on a machine with none (menuAccount.ts).
  const email = useMenuAccount();

  const items: Entry[] = [
    { label: m["home.projects"](), icon: Folders, route: "/" },
    { label: m["home.tasks"](), icon: ListChecks, route: TASKS_ROUTE },
    { label: m["home.libraries"](), icon: LibraryBig, route: LIBRARIES_ROUTE },
    {
      label: m["frozen.title"](),
      icon: Snowflake,
      route: FROZEN_CHAINS_ROUTE,
    },
  ].map((e) => ({ ...e, active: here(e.route) }));
  // The account and the settings — in the header at the title's far right, and on the
  // rail, which has no title, below (docs/userMenu.md).
  const userMenu = (
    <UserMenu
      email={email}
      onSettings={() => navigate(SETTINGS_ROUTE)}
      settingsLabel={m["home.settings"]()}
      label={m["home.account"]()}
      // Beside Settings: the engines, the server's admin page to its admins, About (../../docs/settings.md).
      items={[
        {
          id: "engine",
          label: m["home.engine"](),
          icon: AudioLines,
          onSelect: () => navigate(ENGINE_SETTINGS_ROUTE),
        },
        {
          id: "about",
          label: m["home.about"](),
          icon: Info,
          onSelect: () => navigate(ABOUT_ROUTE),
        },
      ]}
    />
  );

  return (
    <aside
      className={cn(
        "z-20 flex shrink-0 flex-col bg-card",
        away ? "h-full w-52 shadow-2xl" : "w-14 wide:w-52",
      )}
    >
      <div
        className={cn(
          "flex flex-col gap-1 pt-4 pb-3",
          away ? "px-5" : "px-3 wide:px-5",
        )}
      >
        {/* The logo alone on the rail: it is too narrow for the logo and the name together. */}
        <span
          className="flex min-w-0 items-center gap-2 font-brand text-xl leading-none text-white"
          title={VERSION}
        >
          <img src={k7Logo} alt="k7" className="h-6 w-auto shrink-0" />
          <span
            className={cn("truncate", away ? "inline" : "hidden wide:inline")}
          >
            k7
          </span>
          {/* The account ends the header, at the title's far right. */}
          <span
            className={cn(
              "ml-auto shrink-0",
              away ? "flex" : "hidden wide:flex",
            )}
          >
            {userMenu}
          </span>
        </span>
      </div>

      {/* The entries scroll on a short screen; the brand and the user menu stay put. */}
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto pb-3">
        <nav className="flex flex-col gap-1 px-2">
          {items.map((e) => (
            <SidebarEntry key={e.route} entry={e} away={away} />
          ))}
        </nav>
      </div>

      <div
        className={cn(
          "flex items-center gap-1 px-2 pb-3",
          away
            ? "flex-row justify-end"
            : "flex-col wide:flex-row wide:justify-end",
        )}
      >
        {/* First, and only while something comes down (../components/DownloadsButton.tsx). */}
        <DownloadsButton />
        {/* On the rail, which shows no title, the account keeps this stack (../../../docs/projects.md). */}
        <span className={cn(away ? "hidden" : "flex wide:hidden")}>
          {userMenu}
        </span>
      </div>
    </aside>
  );
}
