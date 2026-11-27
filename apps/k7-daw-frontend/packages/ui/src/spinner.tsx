import { LoaderCircle } from "./icons";
import { cn } from "./lib/utils";

const SIZES = { xs: "size-3", sm: "size-4", md: "size-5" } as const;

// Only drawn while something runs: the accent-text loader turning once every 0.9 s.
export function Spinner({
  className,
  label,
  size = "md",
  inherit,
}: {
  className?: string;
  /** The accessible name; without it the spinner is decoration beside a text that already says what runs. */
  label?: string;
  size?: keyof typeof SIZES;
  /** Take the surrounding text colour (inside a button) instead of the accent. */
  inherit?: boolean;
}) {
  return (
    <LoaderCircle
      role={label ? "img" : undefined}
      aria-label={label}
      aria-hidden={label ? undefined : true}
      className={cn(
        SIZES[size],
        "animate-[k7-spin_var(--k7-dur-spin)_linear_infinite]",
        !inherit && "text-primary-text",
        className,
      )}
    />
  );
}

// A spinner and what is loading; the label comes from the caller, the kit has no i18n.
export function Loading({
  label,
  className,
}: {
  label: string;
  className?: string;
}) {
  return (
    <span className={cn("inline-flex items-center gap-2", className)}>
      <Spinner size="sm" inherit />
      {label}
    </span>
  );
}
