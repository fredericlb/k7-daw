import { LoaderCircle } from "./icons";
import { cn } from "./lib/utils";

// Only drawn while something runs: the accent-text loader turning once every 0.9 s.
export function Spinner({
  className,
  label,
}: {
  className?: string;
  /** The accessible name; without it the spinner is decoration beside a text that already says what runs. */
  label?: string;
}) {
  return (
    <LoaderCircle
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cn(
        "size-5 animate-[k7-spin_var(--k7-dur-spin)_linear_infinite] text-primary-text",
        className,
      )}
    />
  );
}
