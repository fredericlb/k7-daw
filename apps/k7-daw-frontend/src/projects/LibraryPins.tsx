// LibraryPins.tsx — what each workspace pins of each library, with the Upgrade that moves a pin to the newest version
// this k7 runs. What a reader gets wrong: Upgrade only rewrites `_library-pins.k7x`; the songs recompile when opened.

import {
  LIBRARY_LEVEL,
  LIBRARY_PINS_FILE,
  type LibraryPins as Pins,
  parseLibraryPins,
  pinnedVersion,
  stringifyLibraryPins,
} from "@studio/song-schema/libraryVersions";
import { Button } from "@studio/ui/button";
import { Chip } from "@studio/ui/chip";
import { RowGroup } from "@studio/ui/row-group";
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

/** One workspace's pins: its file, or none when it names nothing yet. */
async function readPins(f: AccountFiles, workspace: string): Promise<Pins> {
  try {
    const file = await workspaceFiles.read(f, workspace, LIBRARY_PINS_FILE);
    return parseLibraryPins(file.content);
  } catch {
    return {};
  }
}

export function LibraryPins({
  fileService,
  libraries,
}: {
  fileService: AccountFiles;
  libraries: AccountLibrary[];
}) {
  const [rows, setRows] = useState<{ ws: AccountWorkspace; pins: Pins }[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    const all = await accountFiles.workspaces(fileService);
    const own = all.filter((w) => !w.isSingleProject);
    setRows(
      await Promise.all(
        own.map(async (ws) => ({ ws, pins: await readPins(fileService, ws.name) })),
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
      {rows.map(({ ws, pins }) => (
        <section
          key={ws.name}
          aria-label={m["libs.pinsOf"]({ workspace: ws.name })}
          className="flex flex-col gap-2"
        >
          <h2 className="type-caps px-1 text-fg-3">{ws.name}</h2>
          <RowGroup>
            {versioned.map((l) => {
              const pinned = pinnedVersion(pins, l.name);
              const newest = newestRunnable(l);
              const behind =
                typeof pinned === "number" && newest !== null && pinned < newest;
              return (
                <div
                  key={l.name}
                  className="flex min-h-14 flex-wrap items-center gap-2 px-3.5 py-2"
                >
                  <Chip variant="anchor">@{l.name}</Chip>
                  <span className="flex min-w-0 flex-1 flex-wrap items-center gap-1.5">
                    {(l.versions ?? []).map((v) => (
                      <Chip
                        key={v.version}
                        variant="version"
                        newest={v.version === pinned}
                        aria-current={v.version === pinned ? "true" : undefined}
                      >
                        v{v.version}
                        {v.version === newest && ` · ${m["libs.newest"]()}`}
                      </Chip>
                    ))}
                  </span>
                  {behind && (
                    <Button
                      size="sm"
                      variant="glass"
                      disabled={busy !== null}
                      aria-label={m["libs.upgradeTo"]({
                        name: l.name,
                        to: newest,
                      })}
                      onClick={() => void upgrade(ws.name, pins, l.name, newest)}
                    >
                      {m["libs.upgrade"]()}
                    </Button>
                  )}
                </div>
              );
            })}
          </RowGroup>
        </section>
      ))}
    </>
  );
}
