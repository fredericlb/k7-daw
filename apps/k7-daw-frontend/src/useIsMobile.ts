// useIsMobile.ts — is the shell on a phone-sized surface? Width OR height, because a phone
// in landscape passes a width-only test but is far too short for the desktop layout — which
// also makes it testable by resizing a window. Deliberately NOT `pointer: coarse`: a
// touchscreen laptop is not a phone.

import { useEffect, useState } from "react";

const MOBILE_QUERY = "(max-width: 820px), (max-height: 520px)";

export function useIsMobile(): boolean {
  // Read synchronously: correcting in an effect would build the desktop layout's stacked panes for one frame.
  const [isMobile, setIsMobile] = useState(
    () =>
      typeof window !== "undefined" && window.matchMedia(MOBILE_QUERY).matches,
  );
  useEffect(() => {
    const mq = window.matchMedia(MOBILE_QUERY);
    const onChange = () => setIsMobile(mq.matches);
    mq.addEventListener("change", onChange);
    onChange(); // in case it changed between the first render and this effect
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return isMobile;
}

export type ShellLayout = "phone" | "pad" | "desk";

/** Phone is the folded shell; a touch screen that is not folded is a Pad, anything else a Desk. */
export function useShellLayout(): ShellLayout {
  const phone = useIsMobile();
  const [touch, setTouch] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia("(pointer: coarse)").matches,
  );
  useEffect(() => {
    const mq = window.matchMedia("(pointer: coarse)");
    const onChange = () => setTouch(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return phone ? "phone" : touch ? "pad" : "desk";
}

/** Is a finger (no hover) the main pointer? A tablet at any width is, a phone-sized desktop window is not. */
export function useFingerFirst(): boolean {
  const [finger, setFinger] = useState(
    () =>
      typeof window !== "undefined" &&
      window.matchMedia?.("(hover: none)").matches === true,
  );
  useEffect(() => {
    const mq = window.matchMedia?.("(hover: none)");
    if (!mq) return;
    const onChange = () => setFinger(mq.matches);
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return finger;
}
