// modal-focus.ts — an overlay's keyboard contract: Tab stays inside it, and closing gives focus back to the opener.
// The opener is read while the overlay renders, since a child's autofocus has moved focus before any effect runs.
import { type RefObject, useEffect, useRef } from "react";

const TABBABLE =
  'a[href],button:not([disabled]),input:not([disabled]),select:not([disabled]),textarea:not([disabled]),[tabindex]:not([tabindex="-1"])';

export function useModalFocus(
  ref: RefObject<HTMLElement | null>,
  { active = true, trap = true }: { active?: boolean; trap?: boolean } = {},
): void {
  const was = useRef(false);
  const opener = useRef<HTMLElement | null>(null);
  if (active && !was.current)
    opener.current = document.activeElement as HTMLElement | null;
  was.current = active;
  useEffect(() => {
    const el = ref.current;
    if (!active || !el) return;
    const onKey = (e: KeyboardEvent) => {
      if (!trap || e.key !== "Tab") return;
      const all = [...el.querySelectorAll<HTMLElement>(TABBABLE)];
      const first = all[0];
      const last = all[all.length - 1];
      if (!first || !last) return e.preventDefault();
      const at = document.activeElement;
      if (e.shiftKey && (at === first || at === el)) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && at === last) {
        e.preventDefault();
        first.focus();
      }
    };
    el.addEventListener("keydown", onKey);
    return () => {
      el.removeEventListener("keydown", onKey);
      const at = document.activeElement;
      // Only when focus was lost with the overlay: a command that moved it elsewhere keeps it.
      const lost = !at || at === document.body || el.contains(at);
      if (lost && opener.current?.isConnected)
        opener.current.focus({ preventScroll: true });
    };
  }, [active, trap, ref]);
}
