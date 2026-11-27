// LibraryPins.tsx — what each workspace pins of each library, with the Upgrade that moves a pin to the newest version
// this k7 runs. What a reader gets wrong: Upgrade only rewrites `_library-pins.k7x`; the songs recompile when opened.

import { HttpError } from "@studio/http-client/call";
import {
  LIBRARY_LEVEL,
  LIBRARY_PINS_FILE,
  type LibraryPins as Pins,
  parseLibraryPins,
  pinnedVersion,
  stringifyLibraryPins,
} from "@studio/song-schema/libraryVersions";
import { Button } from "@studio/ui/button";
import { Check } from "@studio/ui/icons";
import { RowGroup } from "@studio/ui/row-group";
import { SectionHeading } from "@studio/ui/section-heading";
import type { AccountLibrary, AccountWorkspace } from "@studio/web-protocol";
import { useCallback, useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { accountFiles } from "./api";
import { workspaceFiles } from "./workspaceFiles";
import type { AccountFiles } from "./workspaceOps";

/** The newest published version this k7 can run, or null for a library with none. */
const newestRunnable = (l: AccountLibrary): number | null => {
  const ok = (l.versions ?? []).filter((v) => v.requires <= LIBRARY_LEVEL);
  return ok.length ? Math.max(...ok.map((v) => v.version)) : null;
};

/** One workspace's pins: its file, none when it names nothing yet (only "not found" means that), or why it could not be read. */
async function readPins(
  f: AccountFiles,
  workspace: string,
): Promise<{ pins: Pins; unread: string | null }> {
  try {
    const file = await workspaceFiles.read(f, workspace, LIBRARY_PINS_FILE);
    return { pins: parseLibraryPins(file.content), unread: null };
  } catch (e) {
    if (e instanceof HttpError && e.status === 404)
      return { pins: {}, unread: null };
    return { pins: {}, unread: (e as Error).message };
  }
}

export function LibraryPins({
  fileService,
  libraries,
}: {
  fileService: AccountFiles;
  libraries: AccountLibrary[];
}) {
  const [rows, setRows] = useState<
    { ws: AccountWorkspace; pins: Pins; unread: string | null }[]
  >([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const all = await accountFiles.workspaces(fileService);
    const own = all.filter((w) => !w.isSingleProject);
    setRows(
      await Promise.all(
        own.map(async (ws) => ({
          ws,
          ...(await readPins(fileService, ws.name)),
        })),
      ),
    );
  }, [fileService]);
  useEffect(() => {
    void load().catch((e: Error) => setError(e.message));
  }, [load]);

  const upgrade = async (ws: string, pins: Pins, lib: string, to: number) => {
    setBusy(`${ws}/${lib}`);
    setError(null);
    try {
      await workspaceFiles.write(
        fileService,
        ws,
        LIBRARY_PINS_FILE,
        stringifyLibraryPins({ ...pins, [lib]: to }),
      );
      await load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const versioned = libraries.filter((l) => (l.versions ?? []).length > 0);
  if (!rows.length || !versioned.length) return null;
  return (
    <>
      {error && (
        <p role="alert" className="type-secondary text-error">
          {error}
        </p>
      )}
      <section
        aria-label={m["libs.pinnedBy"]()}
        className="flex flex-col gap-2"
      >
        <SectionHeading as="h2" className="px-1">
          {m["libs.pinnedBy"]()}
        </SectionHeading>
        <RowGroup>
          {rows.flatMap(({ ws, pins, unread }) =>
            unread !== null
              ? [
                  // An unread file is not an empty one: Upgrade would write the other libraries' pins away.
                  <p
                    key={ws.name}
                    role="alert"
                    className="type-secondary px-3.5 py-3 text-error"
                  >
                    {m["libs.pinsUnread"]({
                      workspace: ws.name,
                      reason: unread,
                    })}
                  </p>,
                ]
              : versioned.map((l) => {
                  const pinned = pinnedVersion(pins, l.name);
                  const newest = newestRunnable(l);
                  const behind =
                    typeof pinned === "number" &&
                    newest !== null &&
                    pinned < newest;
                  return (
                    <div
                      key={`${ws.name}/${l.name}`}
                      role="group"
                      aria-label={m["libs.pinsOf"]({ workspace: ws.name })}
                      className="flex min-h-14 flex-wrap items-center gap-2 px-3.5 py-2"
                    >
                      <span className="flex min-w-0 flex-1 flex-col">
                        <span className="type-label truncate">{ws.name}</span>
                        <span className="type-secondary text-fg-2">
                          {behind
                            ? m["libs.upgradeReady"]({
                                name: l.name,
                                from: String(pinned),
                                to: newest,
                              })
                            : m["libs.plays"]({
                                name: l.name,
                                version: String(pinned),
                              })}
                        </span>
                      </span>
                      {behind ? (
                        <Button
                          size="sm"
                          variant="glass"
                          disabled={busy !== null}
                          aria-label={m["libs.upgradeTo"]({
                            name: l.name,
                            to: newest,
                          })}
                          onClick={() =>
                            void upgrade(ws.name, pins, l.name, newest)
                          }
                        >
                          {m["libs.upgrade"]()}
                        </Button>
                      ) : (
                        <span className="type-secondary flex shrink-0 items-center gap-1.5 text-ok">
                          <Check className="size-4" aria-hidden />
                          {m["libs.onNewest"]()}
                        </span>
                      )}
                    </div>
                  );
                }),
          )}
        </RowGroup>
        <p className="type-status px-1 text-fg-3">{m["libs.pinsNote"]()}</p>
      </section>
    </>
  );
}
