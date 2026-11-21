// useRemoteScreens.ts — the screens other devices show of THIS device's project, polled while a project is open.
// A front that answers no LAN family (a server) gives nothing, and the list stays empty.

import { useEffect, useState } from "react";
import { lanApi } from "../surfaces/lanApi";
import { type RemoteScreen, sameScreens } from "../surfaces/paneInUse";
import { pollWhileVisible } from "../visiblePoll";

/** How soon a remote's leaving frees its pane: the door's TTL bounds the rest. */
const POLL_MS = 2000;

const NONE: RemoteScreen[] = [];

export function useRemoteScreens(): RemoteScreen[] {
  const [screens, setScreens] = useState<RemoteScreen[]>(NONE);
  useEffect(() => {
    let live = true;
    const ask = async () => {
      let next = NONE;
      try {
        next = (await lanApi.screens()).screens;
      } catch {
        // Unreachable or not answered: nobody is known to hold a pane, so none is blocked.
      }
      if (live) setScreens((cur) => (sameScreens(cur, next) ? cur : next));
    };
    const stop = pollWhileVisible(ask, POLL_MS, { immediate: true });
    return () => {
      live = false;
      stop();
    };
  }, []);
  return screens;
}
