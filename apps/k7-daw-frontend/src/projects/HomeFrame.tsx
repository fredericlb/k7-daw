// HomeFrame.tsx — what every home page shares: whether the sidebar is folded, and the mini player's
// dock, whose bar is always there at the foot. The engine selector sits above the sidebar's user menu,
// and in the bar while the sidebar is folded. `HomeFrame` is held above the routes, so the fold
// survives a page change; `HomeShell` is each page's layout. See ../../docs/projects.md, "The home bar".

import { AudioLines, PanelLeftClose, PanelLeftOpen } from "@studio/ui/icons";
import { MiniPlayer } from "@studio/ui-mini-player";
import {
  MiniPlayerDock,
  MiniPlayerDockOutlet,
} from "@studio/ui-mini-player/dock";
import {
  createContext,
  type ReactNode,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";
import { HomeEngine } from "../components/HomeEngine";
import { m } from "../paraglide/messages.js";
import {
  HomeCommandBar,
  HomeCommandsButton,
  HomeCommandsSaid,
} from "./HomeCommandBar";
import { HomeSidebar } from "./HomeSidebar";

// Per browser: folding is a habit of this screen, not a setting of the account.
const FOLD_KEY = "k7.homeSidebarFolded";

const readFolded = (): boolean => {
  try {
    return localStorage.getItem(FOLD_KEY) === "1";
  } catch {
    return false;
  }
};

interface Frame {
  folded: boolean;
  setFolded: (folded: boolean) => void;
}

const FrameContext = createContext<Frame>({
  folded: false,
  setFolded: () => {},
});

const useHomeFrame = (): Frame => useContext(FrameContext);

export function HomeFrame({ children }: { children: ReactNode }) {
  const [folded, setFoldedState] = useState(readFolded);
  const setFolded = useCallback((next: boolean) => {
    setFoldedState(next);
    try {
      localStorage.setItem(FOLD_KEY, next ? "1" : "0");
    } catch {
      // A browser refusing storage keeps the fold for this visit only.
    }
  }, []);
  const frame = useMemo(() => ({ folded, setFolded }), [folded, setFolded]);
  // The sidebar's ONE control, at the bar's left edge and at the bottom of the window, where it stays whether
  // the sidebar is out or folded: a fold button that moved would be two buttons to learn.
  const lead = (
    <>
      <button
        type="button"
        onClick={() => setFolded(!folded)}
        title={folded ? m["home.unfoldSidebar"]() : m["home.foldSidebar"]()}
        aria-label={
          folded ? m["home.unfoldSidebar"]() : m["home.foldSidebar"]()
        }
        aria-expanded={!folded}
        className="shrink-0 rounded p-0.5 text-muted-foreground hover:bg-accent hover:text-foreground"
      >
        {folded ? (
          <PanelLeftOpen className="h-4 w-4" />
        ) : (
          <PanelLeftClose className="h-4 w-4" />
        )}
      </button>
      {/* Right of it, as in a project: the command bar's opener (docs/commandBar.md). */}
      <HomeCommandsButton />
    </>
  );
  // What ends every bar a home page docks — a song playing, or nothing: the engine.
  const end = <HomeEngine />;
  return (
    <FrameContext.Provider value={frame}>
      <HomeCommandBar>
        <MiniPlayerDock lead={lead} end={end}>
          {children}
        </MiniPlayerDock>
      </HomeCommandBar>
    </FrameContext.Provider>
  );
}

/** The page frame: the sidebar on the left, the page's own column on the right, the bar under both. */
export function HomeShell({
  actions,
  children,
}: {
  /** Drawn in the bar, before the engine — a page's own control, such as a refresh. */
  actions?: ReactNode;
  children: ReactNode;
}) {
  const { folded } = useHomeFrame();
  return (
    <div className="flex h-full flex-col bg-background text-foreground">
      <div className="flex min-h-0 flex-1">
        {!folded && <HomeSidebar />}
        {/* `contain: layout` holds a page's fixed docks and pop-ins inside its column, clear of the sidebar and the bar. */}
        <div className="flex min-w-0 flex-1 flex-col [contain:layout]">
          {children}
          <HomeCommandsSaid />
        </div>
      </div>
      {/* The mini player of whichever page plays, else the standby bar: always there, the whole width, docked by `HomeFrame`. */}
      <MiniPlayerDockOutlet />
      <MiniPlayer
        standby
        icon={AudioLines}
        // Nothing is said here: the row is the home's controls, and a line about silence is noise in it.
        title=""
        end={
          <>
            {/* The page's own, first: the médiathèque's queue, which the bar holds while nothing plays. */}
            {actions}
            <HomeEngine />
          </>
        }
        playing={false}
        position={0}
        duration={0}
        mediaSession={false}
        labels={{
          region: m["home.player"](),
          more: m["common.more"](),
          play: m["transport.play"](),
          pause: m["transport.pause"](),
          position: m["minimap.movePlayhead"](),
          close: m["common.close"](),
        }}
        onToggle={() => {}}
        onSeek={() => {}}
        transport={null}
        track={null}
      />
    </div>
  );
}
