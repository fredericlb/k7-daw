// presence.ts — a remote tells the main device which surface it shows: a beat every few seconds, and a goodbye
// the moment the page goes. The door forgets a screen that stops beating, so a crashed tablet does not lock a pane.

/** How often the door hears from a screen; its TTL is three beats. */
export const BEAT_MS = 5000;

export interface PresenceOptions {
  /** One beat: PUT the screen's surface and device. */
  beat: () => unknown;
  /** The goodbye, sent as a beacon. */
  bye: () => void;
  ms?: number;
  /** Where `pagehide` is heard; the window. */
  target?: Pick<EventTarget, "addEventListener" | "removeEventListener">;
}

/** Beats now and every `ms` until stopped; says goodbye on `pagehide` and on the stop. Returns the stop. */
export function startPresence(o: PresenceOptions): () => void {
  const target = o.target ?? window;
  let live = true;
  const send = () => {
    if (live) void Promise.resolve(o.beat()).catch(() => {});
  };
  send();
  const timer = setInterval(send, o.ms ?? BEAT_MS);
  const gone = () => o.bye();
  target.addEventListener("pagehide", gone);
  return () => {
    live = false;
    clearInterval(timer);
    target.removeEventListener("pagehide", gone);
    o.bye();
  };
}
