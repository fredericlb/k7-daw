// Section.tsx — the shared furniture of the settings screens: a titled block (a caps heading, a row group, the hint
// under it), a row, the action every row uses and the text field. Here rather than inline, so the screens read as one
// surface and a second section costs a title.

import { Button } from "@studio/ui/button";
import { RowGroup } from "@studio/ui/row-group";
import type { ReactNode } from "react";

/** A text field inside a row: 44 high, the input fill, the focus ring of the kit's inputs. */
export const FIELD =
  "type-mono-value h-11 w-full rounded-(--k7-r-input) border border-(--k7-hairline) bg-(--k7-input-fill) px-3 text-foreground placeholder:text-fg-3 focus:border-lavender focus:shadow-(--k7-focus-ring) focus:outline-none";

export function Section({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: ReactNode;
}) {
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <h2 className="type-caps px-1 text-fg-3">{title}</h2>
      <RowGroup>{children}</RowGroup>
      {hint && <p className="type-status px-1 text-fg-3">{hint}</p>}
    </section>
  );
}

/** One line: what it is on the left, what you can do about it on the right. */
export function Row({
  label,
  detail,
  error,
  children,
}: {
  label: ReactNode;
  detail?: ReactNode;
  /** What went wrong with this row, in the error colour and wrapped, never cut. */
  error?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-h-11 items-center gap-4 px-3.5 py-2.5">
      <div className="min-w-0 flex-1">
        <div className="type-body truncate">{label}</div>
        {detail && !error && (
          <div className="type-status truncate text-fg-3">{detail}</div>
        )}
        {error && (
          <div role="alert" className="type-status text-error">
            {error}
          </div>
        )}
      </div>
      {children && <div className="flex shrink-0 gap-2">{children}</div>}
    </div>
  );
}

/** A disabled action still says WHY where it can: a menu can only grey a row out, a page has room for the sentence. */
export function Action({
  onClick,
  disabled,
  title,
  label,
  children,
  danger,
}: {
  onClick: () => void;
  disabled?: boolean;
  title?: string;
  /** The accessible name where the visible word repeats, such as "Revoke" on each device. */
  label?: string;
  children: ReactNode;
  danger?: boolean;
}) {
  return (
    <Button
      size="sm"
      variant={danger ? "destructive" : "glass"}
      onClick={onClick}
      disabled={disabled}
      title={title}
      aria-label={label}
    >
      {children}
    </Button>
  );
}
