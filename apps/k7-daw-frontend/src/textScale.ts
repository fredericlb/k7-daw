// textScale.ts — follows the iOS text size: the type tokens count in `--k7-u`, which is 1px at the default size.
// WKWebView does not apply Dynamic Type to px text, so the unit is measured off the system font keyword; the
// layout stays in px, only the text grows.

const DEFAULT_BODY = 17; // the system body size at the default text setting, in px

export function followSystemTextSize(): void {
  if (typeof CSS === "undefined" || !CSS.supports("font", "-apple-system-body"))
    return;
  // Desktop Safari has the keyword too, with another default: only a touch screen follows it.
  if (!matchMedia("(pointer: coarse)").matches) return;
  const probe = document.createElement("span");
  probe.style.cssText =
    "position:absolute;visibility:hidden;font:-apple-system-body";
  const apply = () => {
    document.body.append(probe);
    const px = Number.parseFloat(getComputedStyle(probe).fontSize);
    probe.remove();
    if (px > 0)
      document.documentElement.style.setProperty(
        "--k7-u",
        `${px / DEFAULT_BODY}px`,
      );
  };
  apply();
  // The setting changes while the app is away, so it is read again on return.
  document.addEventListener("visibilitychange", () => {
    if (!document.hidden) apply();
  });
}
