// LibrariesNotice.tsx — what the home page says about the libraries, which it no longer edits: one
// line when there is none or the app holds no folder for one, and nothing when all is well.
// The list itself is a page of its own, `/libraries` (LibrariesPage.tsx), read-only.

import { Button } from "@studio/ui/button";
import { LibraryBig, TriangleAlert } from "@studio/ui/icons";
import type { AccountLibrary } from "@studio/web-protocol";
import { Link } from "react-router-dom";
import { m } from "../paraglide/messages.js";
import { LIBRARIES_ROUTE } from "./api";

export function LibrariesNotice({
  libraries,
}: {
  /** `undefined` while unread: no warning before the answer. */
  libraries: AccountLibrary[] | undefined;
}) {
  if (!libraries) return null;
  const said = !libraries.length
    ? m["libs.noticeMissing"]()
    : libraries.some((l) => !l.present)
      ? m["libs.noticeFailed"]()
      : null;
  if (!said) return null;
  return (
    <div className="type-secondary mx-auto flex w-full max-w-5xl items-center gap-3 px-4 pt-4 sm:px-6">
      <div className="flex min-w-0 flex-1 items-center gap-3 rounded-(--k7-r-rows) border border-(--k7-warn-line) bg-(--k7-warn-fill) px-3.5 py-2.5">
        <TriangleAlert className="size-5 shrink-0 text-warn" />
        <span className="flex-1">{said}</span>
        <Button variant="glass" size="sm" asChild>
          <Link to={LIBRARIES_ROUTE}>
            <LibraryBig className="size-5" /> {m["home.libraries"]()}
          </Link>
        </Button>
      </div>
    </div>
  );
}
