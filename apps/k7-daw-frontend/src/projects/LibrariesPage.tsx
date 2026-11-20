// LibrariesPage.tsx — `/libraries`, a page of its own: the bundled libraries, read-only: a library
// changes with an app update.
// A page and not a panel: it is visited on purpose, rarely.
// See ../../docs/projects.md.

import { Toaster } from "@studio/project-shell/toast";
import { Loader2 } from "@studio/ui/icons";
import { useEffect, useState } from "react";
import { AskHostK7 } from "../AskHostK7";
import { m } from "../paraglide/messages.js";
import { findAccountFiles } from "../settings/accountFiles";
import { HomeShell } from "./HomeFrame";
import { LibrariesCard } from "./LibrariesCard";
import { PageFrame } from "./PageFrame";
import type { AccountFiles } from "./workspaceOps";

export function LibrariesPage() {
  // `undefined` while asked: the card says "none answers" only once that is the answer.
  const [fileService, setFileService] = useState<
    AccountFiles | null | undefined
  >(undefined);
  useEffect(() => {
    let live = true;
    void findAccountFiles(false).then((f) => live && setFileService(f));
    return () => {
      live = false;
    };
  }, []);

  return (
    <HomeShell>
      <AskHostK7 />
      <Toaster dismissLabel={m["common.dismiss"]()} />
      <PageFrame title={m["home.libraries"]()} narrow>
        {fileService === undefined ? (
          <div className="flex items-center gap-2 text-muted-foreground text-xs">
            <Loader2 className="h-3.5 w-3.5 animate-spin" />{" "}
            {m["libs.reading"]()}
          </div>
        ) : (
          <LibrariesCard fileService={fileService} inPage />
        )}
      </PageFrame>
    </HomeShell>
  );
}
