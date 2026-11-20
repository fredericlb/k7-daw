// usePhone.ts — is the bar on a phone-sized surface? The bar's breakpoint is 640 px (its `sm:` variants),
// not the shell's: a bar is one row on a tablet, and its host's controls still fit beside it there.

import { useEffect, useState } from "react";

const QUERY = "(max-width: 639px)";

/** Whether the viewport is narrower than the bar's `sm` step. */
export function usePhone(): boolean {
  const [phone, setPhone] = useState(
    () => typeof window !== "undefined" && window.matchMedia(QUERY).matches,
  );
  useEffect(() => {
    const mq = window.matchMedia(QUERY);
    const onChange = () => setPhone(mq.matches);
    mq.addEventListener("change", onChange);
    onChange();
    return () => mq.removeEventListener("change", onChange);
  }, []);
  return phone;
}
