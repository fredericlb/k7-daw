// useAccountFiles.ts — the account's file service for a page that opens no project, asked again while it is silent.
// What a reader gets wrong: `undefined` is "still asking", `null` is the refusal; on a phone the service boots after the page.

import { useEffect, useState } from "react";
import type { AccountFiles } from "../projects/api";
import { findAccountFiles } from "./accountFiles";

/** How often a silent file service is asked again, and how many times — docs/projects.md. */
const ASK_AGAIN_MS = 1500;
const ASK_AGAIN_TIMES = 10;

export function useAccountFiles(
  find: () => Promise<AccountFiles | null> = () => findAccountFiles(false),
): AccountFiles | null | undefined {
  const [files, setFiles] = useState<AccountFiles | null | undefined>(
    undefined,
  );
  // biome-ignore lint/correctness/useExhaustiveDependencies: `find` is a constant of the caller, asked for once.
  useEffect(() => {
    let live = true;
    let asked = 0;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const ask = () => {
      void find().then((f) => {
        if (!live) return;
        if (f === null && asked < ASK_AGAIN_TIMES) {
          asked += 1;
          timer = setTimeout(ask, ASK_AGAIN_MS);
          return;
        }
        setFiles(f);
      });
    };
    ask();
    return () => {
      live = false;
      if (timer) clearTimeout(timer);
    };
  }, []);
  return files;
}
