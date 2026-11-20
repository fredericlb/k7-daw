// LibrariesCard.tsx — the libraries every project inherits, read-only: the named folders of modules
// and chains bundled with the app. A device has no admin and nothing here edits a library; a library
// changes with an app update. See docs/projects.md.

import { LIBRARY_LEVEL } from "@studio/song-schema/libraryVersions";
import { LibraryBig, Loader2, TriangleAlert } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import type { AccountLibrary } from "@studio/web-protocol";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { accountFiles } from "./api";
import type { AccountFiles } from "./workspaceOps";

export function LibrariesCard({
  fileService,
  onLibraries,
  inPage = false,
}: {
  fileService: AccountFiles | null;
  /** Reported UP because it decides more than this card: with no library, no workspace may be made either. */
  onLibraries?: (libraries: AccountLibrary[]) => void;
  /** Under the Libraries page's own heading: a bordered card with no title of its own, rather than the gate's full-width strip. */
  inPage?: boolean;
}) {
  // `undefined` = not read yet, `[]` = read and there is none: a bare install must read 'none', not a loading blank.
  const [libraries, setLibraries] = useState<AccountLibrary[] | undefined>(
    undefined,
  );
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!fileService) return setLibraries([]);
    let live = true;
    void accountFiles
      .libraries(fileService)
      .then((r) => live && setLibraries(r.libraries))
      .catch((e: Error) => {
        setError(e.message);
        setLibraries([]);
      });
    return () => {
      live = false;
    };
  }, [fileService]);

  // Announced UP from the state and not from the setters: what this card holds is the one answer.
  useEffect(() => {
    if (libraries !== undefined) onLibraries?.(libraries);
  }, [libraries, onLibraries]);

  if (libraries === undefined)
    return (
      <div className="flex items-center gap-2 border-b px-6 py-4 text-xs text-muted-foreground">
        <Loader2 className="h-3.5 w-3.5 animate-spin" /> {m["libs.reading"]()}
      </div>
    );

  const missing = !libraries.length;

  return (
    <div
      className={cn(
        "flex flex-col gap-2",
        inPage ? "rounded-lg bg-card p-4" : "border-b px-6 py-4",
        missing &&
          (inPage
            ? "bg-amber-500/10 ring-1 ring-amber-500/40"
            : "border-b-amber-500/40 bg-amber-500/5"),
      )}
    >
      {/* On the page the heading already says Libraries; only the warning is worth a row then. */}
      {(!inPage || missing) && (
        <div className="flex items-center gap-2 text-sm font-medium">
          {missing ? (
            <TriangleAlert className="h-4 w-4 text-amber-500" />
          ) : (
            <LibraryBig className="h-4 w-4" />
          )}
          {missing ? m["libs.needOne"]() : m["libs.title"]()}
        </div>
      )}

      <p className="text-xs text-muted-foreground">
        {missing ? m["libs.missingBody"]() : m["libs.haveBody"]()}
      </p>

      {libraries.map((l) => {
        const versions = l.versions ?? [];
        return (
          <div key={l.name} className="flex flex-col gap-1">
            <div className="flex items-center gap-2 text-xs">
              <code className="flex-1 font-semibold">@{l.name}</code>
              {!l.present && (
                <span className="shrink-0 text-amber-500">
                  {m["libs.notPresent"]()}
                </span>
              )}
            </div>
            <span className="pl-4 text-xs text-muted-foreground">
              {versions.length
                ? m["libs.versions"]({
                    list: versions.map((v) => v.version).join(", "),
                  })
                : m["libs.noVersion"]()}
              {versions
                .filter((v) => v.requires > LIBRARY_LEVEL)
                .map((v) => (
                  <span key={v.version} className="ml-2 text-amber-500">
                    {m["libs.needsLevel"]({
                      version: v.version,
                      level: v.requires,
                    })}
                  </span>
                ))}
            </span>
          </div>
        );
      })}

      {error && (
        <code className="rounded bg-muted px-2 py-1 text-xs text-red-400">
          {error}
        </code>
      )}
    </div>
  );
}
