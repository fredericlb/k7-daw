import { cn } from "./lib/utils";

// The screen's background decor: two blurred blobs low on the screen and two wave strokes at the foot.
// It sits behind everything (z-decor) and takes no pointer.
export function Backdrop({ className }: { className?: string }) {
  return (
    <div
      aria-hidden
      className={cn(
        "pointer-events-none absolute inset-0 z-(--k7-z-decor) overflow-hidden",
        className,
      )}
    >
      <div
        className="absolute -bottom-24 -left-24 size-[420px] rounded-full"
        style={{
          background: "var(--k7-blob-a)",
          opacity: "var(--k7-blob-a-opacity)",
          filter: "var(--k7-blob-a-blur)",
        }}
      />
      <div
        className="absolute -right-28 -bottom-32 size-[460px] rounded-full"
        style={{
          background: "var(--k7-blob-b)",
          opacity: "var(--k7-blob-b-opacity)",
          filter: "var(--k7-blob-b-blur)",
        }}
      />
      <svg
        aria-hidden
        className="absolute inset-x-0 bottom-0 h-28 w-full"
        viewBox="0 0 1200 120"
        preserveAspectRatio="none"
        fill="none"
      >
        <path
          d="M0 80 C 200 30, 400 110, 600 70 S 1000 30, 1200 75"
          stroke="var(--k7-wave-a)"
          strokeWidth="2"
          opacity="0.5"
        />
        <path
          d="M0 100 C 220 60, 420 120, 640 90 S 1000 60, 1200 100"
          stroke="var(--k7-wave-b)"
          strokeWidth="2"
          opacity="0.4"
        />
      </svg>
    </div>
  );
}
