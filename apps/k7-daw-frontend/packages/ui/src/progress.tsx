import { cn } from "./lib/utils";

// A 6 px determinate bar; `value` is 0 to 100, and the bar is a progressbar for assistive tech.
export function Progress({
  value,
  label,
  className,
}: {
  value: number;
  label?: string;
  className?: string;
}) {
  const v = Math.max(0, Math.min(100, value));
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(v)}
      className={cn(
        "h-1.5 w-full overflow-hidden rounded-[3px] bg-(--k7-progress-track)",
        className,
      )}
    >
      <div
        className="h-full rounded-[3px] bg-(--k7-progress-fill) transition-[width] duration-200"
        style={{ width: `${v}%` }}
      />
    </div>
  );
}
