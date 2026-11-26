// listenHere.ts — where THIS browser wants the engine heard: here, through the engine's master
// broadcast, or on the engine's own machine. Per browser, since it is a property of the device you
// sit at; read by the engine link (it subscribes) and by the bar's radios. See ../docs/shell.md.

import { createLiveValue, useLiveValue } from "@studio/recorder/liveValue";

const KEY = "k7.listenHere";

const read = (): boolean => {
  try {
    return localStorage.getItem(KEY) === "1";
  } catch {
    return false;
  }
};

const current = createLiveValue(read());

/** Whether this browser listens here, live. */
export function useListenHere(): boolean {
  return useLiveValue(current);
}
