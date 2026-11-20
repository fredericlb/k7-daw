import { describe, expect, it } from "vitest";
import {
  onToast,
  showToast,
  TOAST_DURATION_MS,
  type ToastItem,
  WARNING_DURATION_MS,
} from "./toast";

function shown(opts: Parameters<typeof showToast>[0]): ToastItem {
  let seen: ToastItem | undefined;
  const off = onToast((t) => {
    seen = t;
  });
  showToast(opts);
  off();
  if (!seen) throw new Error("no toast heard");
  return seen;
}

describe("showToast", () => {
  it("leaves a success on its own, a warning later, and keeps an error", () => {
    expect(shown({ message: "a" }).duration).toBe(TOAST_DURATION_MS);
    expect(shown({ message: "a", variant: "warning" }).duration).toBe(
      WARNING_DURATION_MS,
    );
    expect(shown({ message: "a", variant: "error" }).duration).toBe(0);
  });

  it("keeps a neutral toast with progress until the code dismisses it", () => {
    const scan = shown({
      message: "Scanning plugins",
      variant: "neutral",
      progress: { value: 34, total: 120 },
    });
    expect(scan.duration).toBe(0);
    expect(shown({ message: "Done", variant: "neutral" }).duration).toBe(
      TOAST_DURATION_MS,
    );
  });

  it("places a toast bottom-right unless asked, and answers its id", () => {
    expect(shown({ message: "a" }).placement).toBe("bottom-right");
    expect(shown({ message: "a", placement: "top-centre" }).placement).toBe(
      "top-centre",
    );
    const first = showToast({ message: "x" });
    expect(showToast({ message: "y" })).toBe(first + 1);
  });

  it("carries the action it was given", () => {
    const onClick = () => {};
    const t = shown({
      message: "Engine lost",
      variant: "error",
      action: { label: "Restart", onClick },
    });
    expect(t.action).toEqual({ label: "Restart", onClick });
  });
});
