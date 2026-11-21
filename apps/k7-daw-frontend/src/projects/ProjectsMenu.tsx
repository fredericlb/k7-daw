// ProjectsMenu.tsx — the ⋮ beside the projects page's heading: how the list is shown, starting with
// whether the bundled Examples workspace is in it. See ../../docs/projects.md.
// What reads wrong: the choice is kept per browser only once made; until then it follows the list,
// off as soon as a workspace of one's own is there (`useShowExamples`).

import { Button } from "@studio/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@studio/ui/dropdown-menu";
import { BookOpen, Check, Ellipsis } from "@studio/ui/icons";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { useHomeHelp } from "./homeHelp";

// Per browser, like the sidebar's fold: what a screen shows is a habit, not a setting of the account.
const SHOW_EXAMPLES_KEY = "k7.showExamples";

const readChoice = (): boolean | null => {
  try {
    const v = localStorage.getItem(SHOW_EXAMPLES_KEY);
    return v === "1" ? true : v === "0" ? false : null;
  } catch {
    return null;
  }
};

/** Whether Examples is listed: the browser's choice, else on only while it is all there is. */
export function useShowExamples(
  hasOwn: boolean,
): [boolean, (show: boolean) => void] {
  const [chosen, setChosen] = useState<boolean | null>(readChoice);
  const choose = (show: boolean) => {
    setChosen(show);
    try {
      localStorage.setItem(SHOW_EXAMPLES_KEY, show ? "1" : "0");
    } catch {
      // A browser refusing storage keeps the choice for this visit only.
    }
  };
  return [chosen ?? !hasOwn, choose];
}

export function ProjectsMenu({
  showExamples,
  onShowExamples,
  pinned = false,
}: {
  showExamples: boolean;
  onShowExamples: (show: boolean) => void;
  /** Examples is all there is: it stays shown, and the item says so. */
  pinned?: boolean;
}) {
  const openHelp = useHomeHelp();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="glass"
          size="icon"
          className="shrink-0"
          title={m["repos.pageMenu"]()}
          aria-label={m["repos.pageMenu"]()}
        >
          <Ellipsis className="size-5" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" aria-label={m["repos.pageMenu"]()}>
        {/* A toggle, said as one to a screen reader; the menu stays open so the change is seen. */}
        <DropdownMenuItem
          role="menuitemcheckbox"
          aria-checked={showExamples}
          disabled={pinned}
          disabledReason={pinned ? m["repos.examplesOnly"]() : undefined}
          title={pinned ? m["repos.examplesOnly"]() : undefined}
          onSelect={(e) => {
            e.preventDefault();
            onShowExamples(!showExamples);
          }}
        >
          <Check
            className={`h-4 w-4 ${showExamples ? "" : "invisible"}`}
            aria-hidden
          />
          {m["repos.showExamples"]()}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={openHelp}>
          <BookOpen />
          {m["home.help"]()}
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
