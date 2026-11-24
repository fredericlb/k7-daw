// tab-bar.tsx — the Home tab bar: a floating 66 px pill of 112 x 54 tabs on a phone, a 232 px sidebar of 48 px rows elsewhere.
// A tab is a link (with `href`) or a button; the selected one carries aria-current="page" and the lozenge slides to it.

import * as React from "react";
import { useLozenge } from "./lib/lozenge";
import { cn } from "./lib/utils";

export function TabBar({
  label,
  children,
  trailing,
  className,
}: {
  /** The landmark's accessible name ("Home"). */
  label: string;
  children: React.ReactNode;
  /** The round command-bar button, or anything else that sits beside the bar. */
  trailing?: React.ReactNode;
  className?: string;
}) {
  const ref = React.useRef<HTMLElement | null>(null);
  const box = useLozenge(ref, '[aria-current="page"]');
  return (
    <div className={cn("flex items-center gap-3", className)}>
      <nav
        ref={ref}
        aria-label={label}
        className="float relative flex h-(--k7-tabbar-h) items-center gap-0 rounded-(--k7-r-tabbar) p-[5px]"
      >
        {box ? (
          <span
            aria-hidden
            className="pointer-events-none absolute top-[5px] left-0 h-[54px] rounded-[27px] bg-(--k7-on-fill) shadow-(--k7-highlight) transition-[transform,width] duration-500 ease-(--k7-ease-lozenge)"
            style={{ transform: `translateX(${box.x}px)`, width: box.w }}
          />
        ) : null}
        {children}
      </nav>
      {trailing}
    </div>
  );
}

/** The sidebar form: a floating column down the window's left edge, its rows on top and `foot` at the bottom. */
export function TabSidebar({
  label,
  children,
  foot,
  className,
}: {
  /** The landmark's accessible name ("Home"). */
  label: string;
  children: React.ReactNode;
  /** What sits at the column's foot: the logo and the command-bar button. */
  foot?: React.ReactNode;
  className?: string;
}) {
  const ref = React.useRef<HTMLElement | null>(null);
  const box = useLozenge(ref, '[aria-current="page"]');
  return (
    <div
      className={cn(
        "float flex w-(--k7-sidebar-w) flex-col rounded-[28px] p-3",
        className,
      )}
    >
      <nav
        ref={ref}
        aria-label={label}
        className="relative flex shrink-0 flex-col gap-1"
      >
        {box ? (
          <span
            aria-hidden
            className="pointer-events-none absolute top-0 left-0 h-12 w-full rounded-2xl bg-(--k7-on-fill) shadow-(--k7-highlight) transition-transform duration-500 ease-(--k7-ease-lozenge)"
            style={{ transform: `translateY(${box.y}px)` }}
          />
        ) : null}
        {children}
      </nav>
      <div className="grow" />
      {foot && (
        <div className="flex shrink-0 items-center justify-between pr-0.5 pl-2">
          {foot}
        </div>
      )}
    </div>
  );
}

export function TabBarItem({
  icon,
  label,
  selected,
  href,
  row = false,
  className,
  ...props
}: Omit<React.AnchorHTMLAttributes<HTMLAnchorElement>, "children"> & {
  icon: React.ReactNode;
  label: React.ReactNode;
  selected?: boolean;
  /** A sidebar row: the icon beside the label instead of over it. */
  row?: boolean;
}) {
  const cls = cn(
    "relative z-10 flex cursor-pointer text-fg-2 outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) aria-[current=page]:text-lavender [&_svg]:size-[22px] [&_svg]:shrink-0",
    row
      ? "h-12 items-center gap-3 rounded-2xl px-3.5 text-left"
      : "h-[54px] w-28 flex-col items-center justify-center gap-0.5 rounded-[27px]",
    className,
  );
  const body = (
    <>
      {icon}
      <span className={row ? "text-[15px] font-bold" : "text-xs font-bold"}>
        {label}
      </span>
    </>
  );
  // A tab with a URL is a link, any other a button: the page decides which.
  return href ? (
    <a
      href={href}
      aria-current={selected ? "page" : undefined}
      className={cls}
      {...props}
    >
      {body}
    </a>
  ) : (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      className={cls}
      {...(props as React.ButtonHTMLAttributes<HTMLButtonElement>)}
    >
      {body}
    </button>
  );
}

/** The round button beside the bar, 66 px, or 44 px at the sidebar's foot (square-terminal opens the command bar). */
export function TabBarRound({
  className,
  small = false,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { small?: boolean }) {
  return (
    <button
      type="button"
      className={cn(
        "grid cursor-pointer place-items-center rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring)",
        small
          ? "size-11 border border-(--k7-hairline-strong) bg-(--k7-on-fill) [&_svg]:size-[22px]"
          : "float size-(--k7-tabbar-h) [&_svg]:size-6",
        className,
      )}
      {...props}
    />
  );
}
