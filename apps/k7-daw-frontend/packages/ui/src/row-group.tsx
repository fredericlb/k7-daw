import * as React from "react";
import { ChevronRight } from "./icons";
import { cn } from "./lib/utils";

// A group of rows on one rounded card; a 0.08 hairline splits the rows.
export function RowGroup({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        "row-group overflow-hidden [&>*+*]:border-t [&>*+*]:border-(--k7-hairline-row)",
        className,
      )}
      {...props}
    />
  );
}

interface RowProps {
  /** A 19 px glyph in accent-text; pass the icon element. */
  icon?: React.ReactNode;
  /** The 15 px label. */
  children: React.ReactNode;
  /** A 12 px text-3 line under the label. */
  detail?: React.ReactNode;
  /** What went wrong with this row, in the error colour and wrapped, never cut; it replaces the detail. */
  error?: React.ReactNode;
  /** Anything trailing: a badge, a switch, a value, a few actions. */
  trailing?: React.ReactNode;
  /** A chevron-right marks a row that opens something. */
  disclosure?: boolean;
  /** Lavender text and the 0.08 fill. */
  selected?: boolean;
  /** 52 px for the export rows. */
  tall?: boolean;
  className?: string;
}

const ROW =
  "type-body flex w-full min-h-11 items-center gap-3 px-3.5 py-2.5 text-left text-foreground";

function RowBody({
  icon,
  children,
  detail,
  error,
  trailing,
  disclosure,
}: RowProps) {
  return (
    <>
      {icon ? (
        <span className="grid size-[19px] shrink-0 place-items-center text-primary-text [&_svg]:size-[19px]">
          {icon}
        </span>
      ) : null}
      <span className="min-w-0 flex-1">
        <span className="block truncate">{children}</span>
        {error ? (
          <span role="alert" className="type-status block text-error">
            {error}
          </span>
        ) : detail ? (
          <span className="type-status block truncate text-fg-3">{detail}</span>
        ) : null}
      </span>
      {trailing ? (
        <span className="flex shrink-0 items-center gap-2">{trailing}</span>
      ) : null}
      {disclosure ? (
        <ChevronRight
          className="size-4 shrink-0 text-fg-3"
          strokeWidth={2.2}
          aria-hidden
        />
      ) : null}
    </>
  );
}

const rowClass = (p: Pick<RowProps, "selected" | "tall" | "className">) =>
  cn(
    ROW,
    p.tall && "min-h-[52px]",
    p.selected && "bg-(--k7-row-selected) text-lavender",
    p.className,
  );

type Shown = Omit<React.HTMLAttributes<HTMLDivElement>, keyof RowProps>;

/** A row that only shows something. */
export function Row({
  icon,
  detail,
  error,
  trailing,
  disclosure,
  selected,
  tall,
  className,
  children,
  ...div
}: RowProps & Shown) {
  return (
    <div className={rowClass({ selected, tall, className })} {...div}>
      <RowBody {...{ icon, detail, error, trailing, disclosure }}>
        {children}
      </RowBody>
    </div>
  );
}

type Acting = Omit<
  React.ButtonHTMLAttributes<HTMLButtonElement>,
  keyof RowProps
>;

/** A row that acts: a button with the same look; `selected` also sets aria-pressed. */
export const RowButton = React.forwardRef<HTMLButtonElement, RowProps & Acting>(
  (
    {
      icon,
      detail,
      error,
      trailing,
      disclosure,
      selected,
      tall,
      className,
      children,
      ...btn
    },
    ref,
  ) => (
    <button
      ref={ref}
      type="button"
      aria-pressed={btn.role ? undefined : selected}
      className={cn(
        rowClass({ selected, tall, className }),
        "cursor-pointer outline-none transition-colors hover:bg-on-fill focus-visible:shadow-[inset_0_0_0_2px_var(--k7-focus-line)] disabled:pointer-events-none disabled:opacity-40",
      )}
      {...btn}
    >
      <RowBody {...{ icon, detail, error, trailing, disclosure }}>
        {children}
      </RowBody>
    </button>
  ),
);
RowButton.displayName = "RowButton";
