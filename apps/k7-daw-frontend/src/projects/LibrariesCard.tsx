// LibrariesCard.tsx — the libraries every project inherits, read-only: the named folders of modules
// and chains bundled with the app. A device has no admin and nothing here edits a library; a library
// changes with an app update. See docs/projects.md.

import { LIBRARY_LEVEL } from "@studio/song-schema/libraryVersions";
import { Chip } from "@studio/ui/chip";
import { ErrorCard } from "@studio/ui/error-card";
import { LibraryBig } from "@studio/ui/icons";
import { cn } from "@studio/ui/lib/utils";
import { RowGroup } from "@studio/ui/row-group";
import { Spinner } from "@studio/ui/spinner";
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
      <div className="type-secondary flex items-center gap-2 px-6 py-4 text-fg-2">
        <Spinner /> {m["libs.reading"]()}
      </div>
    );

  const missing = !libraries.length;

  return (
    <div
      className={cn(
        "flex flex-col gap-3",
        inPage ? "" : "border-b border-(--k7-hairline-row) px-6 py-4",
      )}
    >
      {/* On the page the heading already says Libraries; only the warning is worth a row then. */}
      {missing ? (
        <ErrorCard title={m["libs.needOne"]()}>
          {m["libs.missingBody"]()}
        </ErrorCard>
      ) : (
        <>
          {!inPage && (
            <div className="type-label flex items-center gap-2">
              <LibraryBig className="size-5 text-primary-text" />
              {m["libs.title"]()}
            </div>
          )}
          <p
            data-testid="libraries-readonly-note"
            className="type-secondary text-fg-2"
          >
            {m["libs.haveBody"]()}
          </p>
        </>
      )}

      {!missing && (
        <RowGroup>
          {libraries.map((l) => {
            const versions = l.versions ?? [];
            const newest = versions
              .filter((v) => v.requires <= LIBRARY_LEVEL)
              .reduce((n, v) => Math.max(n, v.version), 0);
            return (
              <div
                key={l.name}
                className="flex min-h-14 flex-wrap items-center gap-2 px-3.5 py-2"
              >
                <Chip variant="anchor" orphan={!l.present}>
                  @{l.name}
                </Chip>
                <span className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
                  {versions.length ? (
                    versions.map((v) => (
                      <Chip
                        key={v.version}
                        variant="version"
                        newest={v.version === newest}
                        title={
                          v.requires > LIBRARY_LEVEL
                            ? m["libs.needsLevel"]({
                                version: v.version,
                                level: v.requires,
                              })
                            : undefined
                        }
                      >
                        v{v.version}
                        {v.version === newest && ` · ${m["libs.newest"]()}`}
                      </Chip>
                    ))
                  ) : (
                    <span className="type-secondary text-fg-3">
                      {m["libs.noVersion"]()}
                    </span>
                  )}
                </span>
                {!l.present && (
                  <span className="type-secondary shrink-0 text-warn">
                    {m["libs.notPresent"]()}
                  </span>
                )}
              </div>
            );
          })}
        </RowGroup>
      )}

      {error && <ErrorCard title={error} />}
    </div>
  );
}
