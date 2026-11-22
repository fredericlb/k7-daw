// PageFrame.tsx — the one page frame of the home side (Projects, Tasks, Libraries, Settings): a column of the same
// width on every page, and the top bar: a large title over a count line and the page's own actions. The column ends clear of the floating tab bar. See ../../docs/projects.md.

import { Button } from "@studio/ui/button";
import { ChevronLeft } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import k7Logo from "../../icon/k7.svg";
import { m } from "../paraglide/messages.js";

/** The column every home-side page sits in; the foot leaves room for the tab bar (66 px at 28 from the bottom). */
export const PAGE_COLUMN =
  "mx-auto flex w-full max-w-5xl flex-col gap-5 px-4 pt-6 pb-36 sm:px-6";

/** Back to where the page was reached from, or to Home when it was opened by its address. */
function BackButton() {
  const navigate = useNavigate();
  return (
    <Button
      variant="glass"
      size="icon"
      title={m["common.back"]()}
      aria-label={m["common.back"]()}
      onClick={() =>
        window.history.state?.idx > 0 ? navigate(-1) : navigate("/")
      }
    >
      <ChevronLeft className="size-5" />
    </Button>
  );
}

/** The page's title, an optional line under it, and its actions. */
export function PageHeader({
  title,
  summary,
  actions,
  back = false,
}: {
  title: ReactNode;
  summary?: ReactNode;
  actions?: ReactNode;
  /** A sub-page: a Back button leads the bar. */
  back?: boolean;
}) {
  return (
    <header className="flex items-start justify-between gap-x-4 gap-y-3 max-sm:flex-wrap">
      <div className="flex min-w-0 items-start gap-3">
        {back && <BackButton />}
        <img
          src={k7Logo}
          alt=""
          className="mt-1.5 h-7 w-auto shrink-0"
          draggable={false}
        />
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="type-large-title">{title}</h1>
          {summary && (
            <p className="type-secondary text-fg-2 tabular-nums">{summary}</p>
          )}
        </div>
      </div>
      <div className="flex shrink-0 items-center gap-2">{actions}</div>
    </header>
  );
}

/** A whole page: the scroll, the column, the header, then the body — `narrow` for rows of settings. */
export function PageFrame({
  title,
  summary,
  actions,
  narrow = false,
  back = false,
  children,
}: {
  back?: boolean;
  title: ReactNode;
  summary?: ReactNode;
  actions?: ReactNode;
  narrow?: boolean;
  children: ReactNode;
}) {
  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className={PAGE_COLUMN}>
        <PageHeader
          title={title}
          summary={summary}
          actions={actions}
          back={back}
        />
        <div
          className={cn("flex min-w-0 flex-col gap-4", narrow && "max-w-2xl")}
        >
          {children}
        </div>
      </div>
    </div>
  );
}
