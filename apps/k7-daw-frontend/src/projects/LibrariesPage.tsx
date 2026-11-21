// LibrariesPage.tsx — `/libraries`, titled Library: the bundled libraries with their versions, read-only (a library
// changes with an app update), what each workspace pins of them, with Upgrade, then the account's frozen chains as a
// second section of the same page. See ../../docs/projects.md.

import { Spinner } from "@studio/ui/spinner";
import type { AccountLibrary } from "@studio/web-protocol";
import { useState } from "react";
import { m } from "../paraglide/messages.js";
import { useAccountFiles } from "../settings/useAccountFiles";
import { FrozenChainsSection } from "./FrozenChainsPage";
import { HomeShell } from "./HomeFrame";
import { LibrariesCard } from "./LibrariesCard";
import { LibraryPins } from "./LibraryPins";
import { PageFrame } from "./PageFrame";

export function LibrariesPage() {
  // `undefined` while asked: the card says "none answers" only once that is the answer.
  const fileService = useAccountFiles();
  const [libraries, setLibraries] = useState<AccountLibrary[]>([]);

  return (
    <HomeShell>
      <PageFrame
        title={m["home.library"]()}
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
          <div className="flex flex-col gap-10">
            <div className="flex flex-col gap-5">
              <LibrariesCard
                fileService={fileService}
                onLibraries={setLibraries}
                inPage
              />
              {fileService && (
                <LibraryPins fileService={fileService} libraries={libraries} />
              )}
            </div>
            <FrozenChainsSection files={fileService} />
          </div>
        )}
      </PageFrame>
    </HomeShell>
  );
}
