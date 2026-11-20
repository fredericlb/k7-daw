// tab-bar.tsx — the Home tab bar: a floating 66 px pill of 112 x 54 tabs with a round command-bar button beside it.
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

export function TabBarItem({
  icon,
  label,
  selected,
  href,
  className,
  ...props
}: Omit<React.AnchorHTMLAttributes<HTMLAnchorElement>, "children"> & {
  icon: React.ReactNode;
  label: React.ReactNode;
  selected?: boolean;
}) {
  const cls = cn(
    "relative z-10 flex h-[54px] w-28 cursor-pointer flex-col items-center justify-center gap-0.5 rounded-[27px] text-fg-2 outline-none transition-colors focus-visible:shadow-(--k7-focus-ring) aria-[current=page]:text-lavender [&_svg]:size-[22px]",
    className,
  );
  const body = (
    <>
      {icon}
      <span className="text-xs font-bold">{label}</span>
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

/** The 66 px round button beside the bar (square-terminal opens the command bar). */
export function TabBarRound({
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      type="button"
      className={cn(
        "float grid size-(--k7-tabbar-h) cursor-pointer place-items-center rounded-full text-foreground outline-none focus-visible:shadow-(--k7-focus-ring) [&_svg]:size-6",
        className,
      )}
      {...props}
    />
  );
}
