import { cn } from "./lib/utils";

// A 6 px bar; `value` is 0 to 100, and without one it is indeterminate: a sliding stretch, and no value for assistive tech.
export function Progress({
  value,
  label,
  className,
}: {
  value?: number;
  label?: string;
  className?: string;
}) {
  const known = value !== undefined;
  const v = Math.max(0, Math.min(100, value ?? 0));
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={known ? Math.round(v) : undefined}
      className={cn(
        "h-1.5 w-full overflow-hidden rounded-[3px] bg-(--k7-progress-track)",
        className,
      )}
    >
      <div
        className={cn(
          "h-full rounded-[3px] bg-(--k7-progress-fill)",
          known
            ? "transition-[width] duration-200"
            : "w-1/3 animate-[k7-indeterminate_1.4s_ease-in-out_infinite]",
        )}
        style={known ? { width: `${v}%` } : undefined}
      />
    </div>
  );
}
