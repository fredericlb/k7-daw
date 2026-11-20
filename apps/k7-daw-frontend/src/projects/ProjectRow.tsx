// ProjectRow.tsx — one project in its workspace's card: a LINK first (the kind tile, the name over its status line,
// the kind tag), then its ⋮. Every project here is opened by k7, so there is no app badge. See ../../docs/projects.md.

import { Badge } from "@studio/ui/badge";
import { CornerDownRight } from "@studio/ui/icons";
import type { AccountWorkspace, RepoProject } from "@studio/web-protocol";
import { projectLabel } from "@studio/web-protocol/projects";
import { m } from "../paraglide/messages.js";
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
  workspace: w,
  project: p,
  href,
  targets,
  blocked,
  now,
  locale,
  onCopy,
  onRename,
  onDelete,
}: {
  workspace: AccountWorkspace;
  project: RepoProject;
  href: string;
  /** Where a copy may land. */
  targets: AccountWorkspace[];
  blocked: string;
  /** The clock "edited … ago" is read against, ticked by the list so every row ages together. */
  now: number;
  locale: string;
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
  const status = [
    p.editedAt !== undefined ? editedAgo(p.editedAt, now, locale) : null,
    parent,
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    // The row opens the project as its link does; the link stays for the keyboard and a middle click.
    // biome-ignore lint/a11y/useKeyWithClickEvents lint/a11y/noStaticElementInteractions: the inner link is the keyboard's way in.
    <div
      className="group flex min-h-[60px] cursor-pointer items-center gap-3 px-3.5 py-2 transition-colors [content-visibility:auto] [contain-intrinsic-size:auto_60px] hover:bg-on-fill"
      onClick={() => window.location.assign(href)}
    >
      <a
        className="flex min-w-0 flex-1 items-center gap-3 outline-none focus-visible:shadow-(--k7-focus-ring)"
        aria-label={m["repos.openProjectNamed"]({ name: labelOf(w, p) })}
        title={m["repos.openProject"]({ path: p.path })}
        href={href}
        onClick={(e) => e.stopPropagation()}
      >
        <span
          aria-hidden
          className="grid size-10 shrink-0 place-items-center rounded-xl bg-(--k7-glyph-tile) text-primary-text [&_svg]:size-5"
        >
          <Glyph />
        </span>
        <span className="min-w-0 flex-1">
          <span className="type-body flex items-center gap-2">
            <span className="min-w-0 truncate">{labelOf(w, p)}</span>
            {p.active && (
              <span
                className="size-1.5 shrink-0 rounded-full bg-primary"
                title={m["repos.active"]()}
                role="img"
                aria-label={m["repos.active"]()}
              />
            )}
          </span>
          {status && (
            <span className="type-status block truncate text-fg-3">
              {status}
            </span>
          )}
        </span>
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
        {kind && (
          <Badge variant="tag" className="hidden shrink-0 sm:inline-flex">
            {kind.label()}
          </Badge>
        )}
        {/* On a phone the name needs the row: the tags are a wider screen's. */}
        {p.tags?.map((t) => (
          <Badge key={t} className="hidden shrink-0 md:inline-flex">
            {t}
          </Badge>
        ))}
      </a>
      {/* biome-ignore lint/a11y/useKeyWithClickEvents lint/a11y/noStaticElementInteractions: it only keeps the menu's click off the row. */}
      <span
        className="inline-flex shrink-0 items-center"
        onClick={(e) => e.stopPropagation()}
      >
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
    </div>
  );
}
