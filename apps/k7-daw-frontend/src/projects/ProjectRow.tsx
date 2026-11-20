// ProjectRow.tsx — one project under its workspace's band in the home page's table: a LINK first (the
// archetype's glyph, the name, whether it is the open one), its folder, when it last changed, then its ⋮. There is no app badge: every project here is opened by k7. See ../../docs/projects.md.

import { Badge } from "@studio/ui/badge";
import { CornerDownRight } from "@studio/ui/icons";
import { TableCell, TableRow } from "@studio/ui/table";
import type { AccountWorkspace, RepoProject } from "@studio/web-protocol";
import { projectLabel } from "@studio/web-protocol/projects";
import type { ComponentProps } from "react";
import { m } from "../paraglide/messages.js";
import type { Measured } from "./BandTable";
import { ProjectMenu } from "./ProjectMenu";
import { editedAgo, shapeOfSurfaces } from "./projectKind";
import { shapeWords } from "./shapes";

/** The folder's name without its suffix, or the workspace's when the workspace IS the project. */
const labelOf = (w: AccountWorkspace, p: RepoProject): string =>
  p.path === "."
    ? projectLabel(w.name)
    : projectLabel(p.path.split("/").pop() ?? p.path);

/** The folder holding the project, or null when it sits at the workspace root. */
const parentOf = (p: RepoProject): string | null => {
  const cut = p.path.lastIndexOf("/");
  return cut > 0 ? p.path.slice(0, cut) : null;
};

export function ProjectRow({
  measured,
  workspace: w,
  project: p,
  href,
  targets,
  blocked,
  now,
  locale,
  drop,
  onCopy,
  onRename,
  onDelete,
}: {
  measured: Measured;
  workspace: AccountWorkspace;
  project: RepoProject;
  href: string;
  /** Where a copy may land. */
  targets: AccountWorkspace[];
  blocked: string;
  /** The clock "edited … ago" is read against, ticked by the list so every row ages together. */
  now: number;
  locale: string;
  /** The workspace's drop handlers and its lit state, which every row of it shares. */
  drop: { className: string } & Pick<
    ComponentProps<"tr">,
    "onDragOver" | "onDragLeave" | "onDrop"
  >;
  onCopy: (
    to: AccountWorkspace,
    folder: string,
    onStep: (s: string) => void,
  ) => Promise<void>;
  onRename: (folder: string, onStep: (s: string) => void) => Promise<void>;
  onDelete: (onStep: (s: string) => void) => Promise<void>;
}) {
  const label = p.path === "." ? w.name : labelOf(w, p);
  const shape = shapeOfSurfaces(p.surfaces);
  const kind = shape ? shapeWords(shape) : null;
  const Glyph = kind?.icon ?? CornerDownRight;
  const parent = parentOf(p);
  const { className: lit, ...dropHandlers } = drop;
  return (
    // The row opens the project as its link does; the link stays for the keyboard and a middle click.
    <TableRow
      {...measured}
      {...dropHandlers}
      data-workspace={w.name}
      className={`group cursor-pointer ${lit}`}
      onClick={() => window.location.assign(href)}
    >
      <TableCell className="pl-8">
        <a
          className="flex min-w-0 items-center gap-2.5 text-sm"
          title={m["repos.openProject"]({ path: p.path })}
          href={href}
          onClick={(e) => e.stopPropagation()}
        >
          <Glyph
            className="h-4 w-4 shrink-0 text-muted-foreground group-hover:text-foreground"
            aria-label={kind?.label()}
          />
          <span className="min-w-16 truncate">{labelOf(w, p)}</span>
          {p.active && (
            <span
              className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary"
              title={m["repos.active"]()}
              role="img"
              aria-label={m["repos.active"]()}
            />
          )}
          {/* A demo is overwritten on the next start: said before anyone edits one and loses it. */}
          {p.demo && (
            <Badge
              variant="amber"
              className="hidden shrink-0 sm:inline-flex"
              title={m["demo.badgeHint"]()}
            >
              {m["demo.badge"]()}
            </Badge>
          )}
          {/* On a phone the name needs the row: the badges are a wider screen's. */}
          {p.tags?.map((t) => (
            <Badge key={t} className="hidden shrink-0 sm:inline-flex">
              {t}
            </Badge>
          ))}
        </a>
      </TableCell>
      <TableCell className="hidden truncate text-muted-foreground text-xs md:table-cell">
        {parent}
      </TableCell>
      <TableCell className="hidden text-muted-foreground text-xs tabular-nums sm:table-cell">
        {p.editedAt !== undefined ? editedAgo(p.editedAt, now, locale) : null}
      </TableCell>
      <TableCell className="text-right" onClick={(e) => e.stopPropagation()}>
        <span className="inline-flex items-center justify-end gap-1">
          <ProjectMenu
            path={p.path}
            label={label}
            app={p.app}
            blocked={blocked}
            targets={targets}
            onCopy={onCopy}
            onRename={onRename}
            onDelete={onDelete}
          />
        </span>
      </TableCell>
    </TableRow>
  );
}
