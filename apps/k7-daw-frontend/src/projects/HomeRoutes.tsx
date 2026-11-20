// HomeRoutes.tsx — the pages with no project in the address, as ROUTES so going between them reloads nothing, with
// the home frame (the command bar, the toasts) held above them so it outlives every one. A chunk of its own: a
// project page never loads these pages (../../docs/shell.md).

import { lazy, type ReactNode, Suspense } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import {
  ABOUT_ROUTE,
  ENGINE_SETTINGS_ROUTE,
  FROZEN_CHAINS_ROUTE,
  LIBRARIES_ROUTE,
  SETTINGS_ROUTE,
  TASKS_ROUTE,
} from "./api";
import { Home } from "./Home";
import { HomeFrame } from "./HomeFrame";

/** The home's other pages, fetched when visited: the médiathèque carries the editor stack the login and home pages must not wait for (docs/audit.md). */
const LibrariesPage = lazy(() =>
  import("./LibrariesPage").then((m) => ({
    default: m.LibrariesPage,
  })),
);
const FrozenChainsPage = lazy(() =>
  import("./FrozenChainsPage").then((m) => ({
    default: m.FrozenChainsPage,
  })),
);
const TasksPage = lazy(() =>
  import("./TasksPage").then((m) => ({ default: m.TasksPage })),
);
const EnginePage = lazy(() =>
  import("../settings/EnginePage").then((m) => ({
    default: m.EnginePage,
  })),
);
const SettingsPage = lazy(() =>
  import("../settings/SettingsPage").then((m) => ({
    default: m.SettingsPage,
  })),
);

export function HomeRoutes({ fallback }: { fallback: ReactNode }) {
  return (
    // The command bar and the toasts, shared by every page below (HomeFrame.tsx).
    <HomeFrame>
      <Suspense fallback={fallback}>
        <Routes>
          <Route path={LIBRARIES_ROUTE} element={<LibrariesPage />} />
          <Route path={FROZEN_CHAINS_ROUTE} element={<FrozenChainsPage />} />
          <Route path={SETTINGS_ROUTE} element={<SettingsPage />} />
          <Route path={ENGINE_SETTINGS_ROUTE} element={<EnginePage />} />
          {/* About lives in App settings now; an old address lands there. */}
          <Route
            path={ABOUT_ROUTE}
            element={<Navigate to={SETTINGS_ROUTE} replace />}
          />
          <Route path={TASKS_ROUTE} element={<TasksPage />} />
          <Route path="*" element={<Home />} />
        </Routes>
      </Suspense>
    </HomeFrame>
  );
}
