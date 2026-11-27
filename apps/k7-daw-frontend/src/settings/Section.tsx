// Section.tsx — the shared furniture of the settings screens: a titled block (a caps heading, a row group, the hint
// under it) and the action every row uses. Here rather than inline, so the screens read as one
// surface and a second section costs a title.

import { Button } from "@studio/ui/button";
import { RowGroup } from "@studio/ui/row-group";
import { SectionHeading } from "@studio/ui/section-heading";
import type { ReactNode } from "react";

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
      <SectionHeading as="h2" className="px-1">
        {title}
      </SectionHeading>
      <RowGroup>{children}</RowGroup>
      {hint && <p className="type-status px-1 text-fg-3">{hint}</p>}
    </section>
  );
}

/** A disabled action still says WHY where it can: a menu can only grey a row out, a page has room for the sentence. */
export function Action({
  onClick,
  disabled,
  title,
  label,
  pressed,
  disabledReason,
  children,
  danger,
}: {
  onClick: () => void;
  disabled?: boolean;
  title?: string;
  /** The accessible name where the visible word repeats, such as "Revoke" on each device. */
  label?: string;
  /** A choice among several: the chosen one reads as lit, not as unavailable. */
  pressed?: boolean;
  /** Why it is disabled; a press then shows it in a toast. */
  disabledReason?: string;
  children: ReactNode;
  danger?: boolean;
}) {
  return (
    <Button
      size="sm"
      variant={danger ? "destructive" : "glass"}
      onClick={onClick}
      disabled={disabled}
      disabledReason={disabledReason}
      title={title}
      aria-label={label}
      aria-pressed={pressed}
    >
      {children}
    </Button>
  );
}
