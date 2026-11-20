// LibrariesPage.tsx — `/libraries`: the bundled libraries with their versions, read-only (a library changes with an
// app update), then what each workspace pins of them, with Upgrade. A page and not a panel: it is visited on purpose,
// rarely. See ../../docs/projects.md.

import { Spinner } from "@studio/ui/spinner";
import type { AccountLibrary } from "@studio/web-protocol";
import { useEffect, useState } from "react";
import { m } from "../paraglide/messages.js";
import { findAccountFiles } from "../settings/accountFiles";
import { HomeShell } from "./HomeFrame";
import { LibrariesCard } from "./LibrariesCard";
import { LibraryPins } from "./LibraryPins";
import { PageFrame } from "./PageFrame";
import type { AccountFiles } from "./workspaceOps";

export function LibrariesPage() {
  // `undefined` while asked: the card says "none answers" only once that is the answer.
  const [fileService, setFileService] = useState<
    AccountFiles | null | undefined
  >(undefined);
  const [libraries, setLibraries] = useState<AccountLibrary[]>([]);
  useEffect(() => {
    let live = true;
    void findAccountFiles(false).then((f) => live && setFileService(f));
    return () => {
      live = false;
    };
  }, []);

  return (
    <HomeShell>
      <PageFrame
        title={m["home.libraries"]()}
        summary={
          fileService && libraries.length > 0
            ? (libraries.length === 1
                ? m["libs.summary.one"]
                : m["libs.summary.other"])({ n: libraries.length })
            : undefined
        }
        narrow
      >
        {fileService === undefined ? (
          <div className="type-secondary flex items-center gap-2 text-fg-2">
            <Spinner /> {m["libs.reading"]()}
          </div>
        ) : (
          <>
            <LibrariesCard
              fileService={fileService}
              onLibraries={setLibraries}
              inPage
            />
            {fileService && (
              <LibraryPins fileService={fileService} libraries={libraries} />
            )}
          </>
        )}
      </PageFrame>
    </HomeShell>
  );
}
